//! Processos CPL3, round-robin, IPC por instancia e mecanismo de reinicio.
//! Todos os estados globais sao acessados com IF=0: bootstrap ou interrupt gate.
//! O supervisor decide quando recuperar; o nucleo aplica autoridade e isolamento.
use crate::{
    cpu::{self, Quadro},
    memoria, plataforma,
};
use contrato_boot::InformacoesBoot;
use contrato_sistema::{
    chamada, criar_referencia, erro, estado, id_referencia, Mensagem, INFO_USUARIO,
    QUANTIDADE_SERVICOS, REDE, TOPO_PILHA,
};
use core::{
    fmt::Write,
    mem,
    ptr::{addr_of_mut, write, write_bytes},
};
use nucleo::controle::{
    autorizar_gerenciamento, localizar_instancia, pode_enviar, proxima_instancia_rede,
    CaixaMensagens,
};
static TERMINAL_ELF: &[u8] = include_bytes!(env!("RAIZ_TERMINAL"));
static REDE_ELF: &[u8] = include_bytes!(env!("RAIZ_REDE"));
static SUPERVISOR_ELF: &[u8] = include_bytes!(env!("RAIZ_SUPERVISOR"));
const PILHA_KERNEL: usize = 64 * 1024;
#[repr(C, align(4096))]
#[derive(Clone, Copy)]
struct Pilha([u8; PILHA_KERNEL]);
static mut PILHAS: [Pilha; QUANTIDADE_SERVICOS] = [Pilha([0; PILHA_KERNEL]); QUANTIDADE_SERVICOS];
#[derive(Clone, Copy)]
struct Processo {
    estado: u64,
    referencia: u64,
    cr3: u64,
    fisico: u64,
    pilha: u64,
    quadro: *mut Quadro,
    fila: CaixaMensagens,
}
const VAZIO: Processo = Processo {
    estado: estado::INEXISTENTE,
    referencia: 0,
    cr3: 0,
    fisico: 0,
    pilha: 0,
    quadro: core::ptr::null_mut(),
    fila: CaixaMensagens::NOVA,
};
static mut PROCESSOS: [Processo; QUANTIDADE_SERVICOS] = [VAZIO; QUANTIDADE_SERVICOS];
static mut ATUAL: usize = 0;
static mut TIQUES: u64 = 0;
// As regioes apontadas pelo contrato de boot permanecem reservadas neste MVP.
static mut BOOT: Option<InformacoesBoot> = None;

pub unsafe fn iniciar(boot: &InformacoesBoot, mmio: Option<u64>) -> Result<(), &'static str> {
    BOOT = Some(*boot);
    for (indice, elf) in [TERMINAL_ELF, REDE_ELF, SUPERVISOR_ELF]
        .into_iter()
        .enumerate()
    {
        let referencia =
            criar_referencia(indice as u64 + 1, 1).ok_or("identidade inicial invalida")?;
        let imagem = memoria::carregar(indice, elf, boot, mmio, referencia)?;
        let processo = novo_contexto(indice, referencia, &imagem);
        write(
            addr_of_mut!(PROCESSOS).cast::<Processo>().add(indice),
            processo,
        );
        let _ = writeln!(crate::io::Emergencia,
            "[microkernel] SERVICO_CARREGADO id={} referencia={:#x} ELF={} bytes fisico={:#x} CR3={:#x}",
            indice + 1, referencia, imagem.tamanho, imagem.fisico, imagem.cr3);
    }
    Ok(())
}

/// A antiga instancia deste indice nao esta executando nem voltara a executar.
unsafe fn novo_contexto(indice: usize, referencia: u64, imagem: &memoria::Imagem) -> Processo {
    let base = addr_of_mut!(PILHAS).cast::<Pilha>().add(indice);
    write_bytes(base, 0, 1);
    let pilha = base as u64 + PILHA_KERNEL as u64;
    let quadro = (pilha - mem::size_of::<Quadro>() as u64) as *mut Quadro;
    write(
        quadro,
        Quadro {
            rip: imagem.entrada,
            cs: 0x23,
            ss: 0x1b,
            rflags: 0x202,
            rsp: TOPO_PILHA - 8,
            rdi: INFO_USUARIO,
            ..Quadro::default()
        },
    );
    Processo {
        estado: estado::EXECUTAVEL,
        referencia,
        cr3: imagem.cr3,
        fisico: imagem.fisico,
        pilha,
        quadro,
        fila: CaixaMensagens::NOVA,
    }
}

pub unsafe fn executar() -> ! {
    ATUAL = 0;
    let p = PROCESSOS[0];
    cpu::selecionar(p.pilha, true, p.cr3);
    cpu::executar(p.quadro)
}
unsafe fn agendar() -> *mut Quadro {
    for deslocamento in 1..=QUANTIDADE_SERVICOS {
        let indice = (ATUAL + deslocamento) % QUANTIDADE_SERVICOS;
        let p = PROCESSOS[indice];
        if p.estado == estado::EXECUTAVEL {
            ATUAL = indice;
            cpu::selecionar(p.pilha, indice == 0, p.cr3);
            return p.quadro;
        }
    }
    crate::io::serial_texto("[microkernel] Nenhum servico executavel.\n");
    crate::io::parar()
}
unsafe fn referencias_atuais() -> [u64; QUANTIDADE_SERVICOS] {
    [
        PROCESSOS[0].referencia,
        PROCESSOS[1].referencia,
        PROCESSOS[2].referencia,
    ]
}
unsafe fn resolver(id: u64) -> u64 {
    if !(1..=QUANTIDADE_SERVICOS as u64).contains(&id) {
        return erro::SERVICO;
    }
    let referencia = PROCESSOS[id as usize - 1].referencia;
    if referencia == 0 {
        erro::SERVICO
    } else {
        referencia
    }
}
unsafe fn consultar_estado(referencia: u64) -> u64 {
    match localizar_instancia(referencia, &referencias_atuais()) {
        Ok(indice) => PROCESSOS[indice].estado,
        Err(e) => e,
    }
}

#[no_mangle]
pub unsafe extern "C" fn despachar_interrupcao(q: *mut Quadro) -> *mut Quadro {
    let f = &mut *q;
    if f.cs & 3 != 3 {
        let _ = writeln!(
            crate::io::Emergencia,
            "\n[microkernel] FALHA_NUCLEO vetor={} erro={:#x} RIP={:#x} CR2={:#x} CR3={:#x}",
            f.vetor,
            f.erro,
            f.rip,
            cpu::cr2(),
            cpu::cr3()
        );
        crate::io::parar();
    }
    PROCESSOS[ATUAL].quadro = q;
    match f.vetor {
        32 => {
            TIQUES = TIQUES.wrapping_add(1);
            cpu::confirmar_irq0();
            agendar()
        }
        128 => {
            match f.rax {
                chamada::CEDER => {
                    f.rax = 0;
                    return agendar();
                }
                chamada::ENVIAR => f.rax = enviar(f),
                chamada::RECEBER => receber(f),
                chamada::SAIR => {
                    let id = ATUAL as u64 + 1;
                    PROCESSOS[ATUAL].estado = estado::FINALIZADO;
                    if id == REDE {
                        plataforma::desativar_rede();
                    }
                    let _ = writeln!(
                        crate::io::Emergencia,
                        "[microkernel] SERVICO_FINALIZADO id={} referencia={:#x}",
                        id,
                        PROCESSOS[ATUAL].referencia
                    );
                    return agendar();
                }
                chamada::PCI_LER => f.rax = plataforma::ler(ATUAL as u64 + 1, f.rdi, f.rsi),
                chamada::PCI_COMANDO => {
                    f.rax = plataforma::escrever_comando(ATUAL as u64 + 1, f.rdi, f.rsi)
                }
                chamada::ESTADO_SERVICO => f.rax = consultar_estado(f.rdi),
                chamada::TIQUES => f.rax = TIQUES,
                chamada::RESOLVER => f.rax = resolver(f.rdi),
                chamada::REINICIAR => f.rax = reiniciar(f.rdi),
                chamada::ENCERRAR => f.rax = encerrar(f.rdi),
                _ => f.rax = erro::CHAMADA,
            }
            q
        }
        _ => {
            let id = ATUAL as u64 + 1;
            PROCESSOS[ATUAL].estado = estado::FALHOU;
            if id == REDE {
                plataforma::desativar_rede();
            }
            let _ = writeln!(crate::io::Emergencia,
                "\n[microkernel] SERVICO_FALHOU id={} vetor={} erro={:#x} RIP={:#x} CR2={:#x} referencia={:#x}",
                id, f.vetor, f.erro, f.rip, cpu::cr2(), PROCESSOS[ATUAL].referencia);
            agendar()
        }
    }
}

unsafe fn enviar(f: &Quadro) -> u64 {
    let destino = match localizar_instancia(f.rdi, &referencias_atuais()) {
        Ok(i) => i,
        Err(e) => return e,
    };
    let origem_id = ATUAL as u64 + 1;
    if !pode_enviar(origem_id, id_referencia(f.rdi)) {
        return erro::PERMISSAO;
    }
    if f.rsi == 0 {
        return erro::CHAMADA;
    }
    let origem = PROCESSOS[ATUAL].referencia;
    let p = &mut *addr_of_mut!(PROCESSOS).cast::<Processo>().add(destino);
    if p.estado != estado::EXECUTAVEL {
        return erro::SERVICO;
    }
    match p.fila.inserir(Mensagem {
        origem,
        codigo: f.rsi,
        a: f.rdx,
        b: f.r10,
        c: f.r8,
        d: f.r9,
    }) {
        Ok(()) => 0,
        Err(e) => e,
    }
}
unsafe fn receber(f: &mut Quadro) {
    let atuais = referencias_atuais();
    let p = &mut *addr_of_mut!(PROCESSOS).cast::<Processo>().add(ATUAL);
    let mut mensagem = Mensagem::default();
    while let Some(m) = p.fila.retirar() {
        // Defesa adicional: uma referencia revogada nunca chega a userspace,
        // mesmo se outra rotina de limpeza for adicionada incorretamente no futuro.
        if localizar_instancia(m.origem, &atuais).is_ok() {
            mensagem = m;
            break;
        }
    }
    f.rax = mensagem.codigo;
    f.rdi = mensagem.origem;
    f.rsi = mensagem.a;
    f.rdx = mensagem.b;
    f.r10 = mensagem.c;
    f.r8 = mensagem.d;
    f.r9 = 0;
}

/// Descarta pedidos destinados ao processo e suas respostas ainda nao recebidas.
/// Mensagens de outros remetentes mantem sua ordem, mesmo com a fila circular cheia.
unsafe fn purgar_instancia(indice: usize, antiga: u64) -> usize {
    let mut removidas = 0;
    for n in 0..QUANTIDADE_SERVICOS {
        let p = &mut *addr_of_mut!(PROCESSOS).cast::<Processo>().add(n);
        removidas += if n == indice {
            p.fila.limpar()
        } else {
            p.fila.descartar_origem(antiga)
        };
    }
    removidas
}

unsafe fn reiniciar(antiga: u64) -> u64 {
    let indice = match autorizar_gerenciamento(ATUAL as u64 + 1, antiga, &referencias_atuais()) {
        Ok(i) => i,
        Err(e) => return e,
    };
    if !matches!(
        PROCESSOS[indice].estado,
        estado::EXECUTAVEL | estado::FALHOU | estado::FINALIZADO
    ) {
        return erro::ESTADO;
    }
    // A recusa por quota precede qualquer mudanca no processo ou no dispositivo.
    let nova = match proxima_instancia_rede(antiga) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let Some(boot) = BOOT else {
        return erro::ESTADO;
    };
    // Preparacao em arena E tabelas ainda nao publicadas. Falha preserva a instancia
    // anterior, suas filas, seu contexto e o estado de bus mastering.
    let imagem =
        match memoria::carregar(indice, REDE_ELF, &boot, plataforma::mmio_atribuido(), nova) {
            Ok(i) => i,
            Err(e) => {
                let _ = writeln!(
                    crate::io::Emergencia,
                    "[microkernel] REINICIO_RECUSADO referencia={:#x} motivo={}",
                    antiga,
                    e
                );
                return erro::IMAGEM;
            }
        };
    let arena_antiga = PROCESSOS[indice].fisico;
    plataforma::desativar_rede();
    PROCESSOS[indice].estado = estado::FINALIZADO;
    let purgadas = purgar_instancia(indice, antiga);
    // O chamador e o supervisor. A pilha substituida pertence a rede inativa,
    // e a antiga arena de usuario permanece intocada, em quarentena ate reboot.
    let processo = novo_contexto(indice, nova, &imagem);
    write(
        addr_of_mut!(PROCESSOS).cast::<Processo>().add(indice),
        processo,
    );
    let _ = writeln!(crate::io::Emergencia,
        "[microkernel] SERVICO_REINICIADO id=2 antiga={:#x} nova={:#x} arena_antiga={:#x} arena_nova={:#x} purgadas={}",
        antiga, nova, arena_antiga, imagem.fisico, purgadas);
    let _ = writeln!(
        crate::io::Emergencia,
        "[microkernel] DMA_QUARENTENA referencia={:#x} arena={:#x} bytes={}",
        antiga,
        arena_antiga,
        contrato_sistema::TAMANHO_USUARIO
    );
    nova
}

unsafe fn encerrar(referencia: u64) -> u64 {
    let indice = match autorizar_gerenciamento(ATUAL as u64 + 1, referencia, &referencias_atuais())
    {
        Ok(i) => i,
        Err(e) => return e,
    };
    if PROCESSOS[indice].estado != estado::EXECUTAVEL {
        return erro::ESTADO;
    }
    plataforma::desativar_rede();
    PROCESSOS[indice].estado = estado::FINALIZADO;
    let purgadas = purgar_instancia(indice, referencia);
    let _ = writeln!(
        crate::io::Emergencia,
        "[microkernel] SERVICO_ENCERRADO id=2 referencia={:#x} purgadas={}",
        referencia,
        purgadas
    );
    0
}
