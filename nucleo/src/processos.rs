//! Processos CPL3, round-robin, IPC por instancia e mecanismo de reinicio.
//! Todos os estados globais sao acessados com IF=0: bootstrap ou interrupt gate.
//! O supervisor decide quando recuperar; o nucleo aplica autoridade e isolamento.
use crate::{
    cpu::{self, Quadro},
    memoria, plataforma,
};
use contrato_boot::InformacoesBoot;
#[cfg(feature = "teste-falha-preparacao")]
use contrato_sistema::geracao_referencia;
use contrato_sistema::{
    chamada, classe_evento, criar_referencia, erro, id_referencia, prazo, referencia_valida,
    Mensagem, INFO_USUARIO, QUANTIDADE_SERVICOS, REDE, SUPERVISOR, TERMINAL, TOPO_PILHA,
};
use core::{
    fmt::Write,
    mem,
    ptr::{addr_of_mut, write, write_bytes},
};
use nucleo::agendamento::{
    calcular_prazo, cancelar_espera, expirar_recepcao, invalidar_espera_por_alvo, EsperaIpc,
    Estado, Vida,
};
use nucleo::controle::{
    autorizar_gerenciamento, inserir_e_despertar, inserir_terminal_e_despertar,
    localizar_instancia, pode_enviar, preparar_recepcao, preparar_reinicio, retirar_atual,
    CaixaMensagens,
};
use nucleo::irq::Irq;
use nucleo::observacao::{selecionar, EventoEscolhido, Observacao};
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
    estado: Estado,
    referencia: u64,
    cr3: u64,
    fisico: u64,
    pilha: u64,
    quadro: *mut Quadro,
    fila: CaixaMensagens,
    espera_ipc: EsperaIpc,
    recepcao_pendente: bool,
    prazo_recepcao: Option<u64>,
    alvo_observado: u64,
    observacao: Observacao,
    espera_evento: bool,
    preferir_vida: bool,
    espera_irq: bool,
    preferir_irq: bool,
}
const VAZIO: Processo = Processo {
    estado: Estado::VAZIO,
    referencia: 0,
    cr3: 0,
    fisico: 0,
    pilha: 0,
    quadro: core::ptr::null_mut(),
    fila: CaixaMensagens::NOVA,
    espera_ipc: EsperaIpc::NOVA,
    recepcao_pendente: false,
    prazo_recepcao: None,
    alvo_observado: 0,
    observacao: Observacao::NOVA,
    espera_evento: false,
    preferir_vida: false,
    espera_irq: false,
    preferir_irq: false,
};
static mut PROCESSOS: [Processo; QUANTIDADE_SERVICOS] = [VAZIO; QUANTIDADE_SERVICOS];
static mut ATUAL: usize = 0;
// Pilha independente das instancias, que podem ser substituidas.
static mut PILHA_OCIOSA: Pilha = Pilha([0; PILHA_KERNEL]);
static mut OCIOSO: bool = false;
// Contadores do ensaio RZ-024. Acesso somente com IF=0 (bootstrap/interrupt
// gate), CPU única. Não são usados pelo caminho normal nem alteram escalonamento.
#[cfg(feature = "teste-rz024")]
static mut TESTE_RZ024_ENTRADAS_IDLE: u32 = 0;
#[cfg(feature = "teste-rz024")]
static mut TESTE_RZ024_TIQUES_IDLE: u32 = 0;
#[cfg(feature = "teste-rz024")]
static mut TESTE_RZ024_IRQ4_IDLE: u32 = 0;
#[cfg(feature = "teste-rz024")]
static mut TESTE_RZ024_REFERENCIAS: [u64; QUANTIDADE_SERVICOS] = [0; QUANTIDADE_SERVICOS];
#[cfg(feature = "teste-rz024")]
static mut TESTE_RZ024_EPISODIO_ATIVO: bool = false;
#[cfg(feature = "teste-rz024")]
static mut TESTE_RZ024_ARMADO: bool = false;
#[cfg(feature = "teste-idle")]
static mut TESTE_RETORNO_REGISTRADO: bool = false;
#[cfg(feature = "teste-prazo-idle")]
static mut TESTE_PRAZO_IDLE_IRQ_REGISTRADA: bool = false;
static mut TIQUES: u64 = 0;
// Apenas bootstrap e entradas com IF=0 acessam este slot na CPU unica. Cada
// referencia temporaria abaixo termina antes de outra chamada ou reagendamento.
static mut IRQ4: Irq = Irq::NOVA;
// Instrumentacao exclusiva da imagem de ensaio; IF=0 serializa acesso.
#[cfg(feature = "teste-irq")]
static mut TESTE_IRQ4_ENTRADAS: u64 = 0;
// IF=0 serializa a instrumentacao opt-in na CPU unica.
#[cfg(feature = "teste-uart")]
static mut TESTE_UART_BLOQUEIO_REGISTRADO: bool = false;
// O diagnostico e opt-in; IF=0 e a CPU unica serializam essas flags. Elas
// controlam somente dois registros do seletor, sem atuar sobre fila ou IRQ.
#[cfg(feature = "teste-seletor-irq-ipc")]
static mut TESTE_IRQ_IPC_TRACE_ATIVO: bool = false;
#[cfg(feature = "teste-seletor-irq-ipc")]
static mut TESTE_IRQ_IPC_TRACE_RESTANTE: u8 = 0;
// Registro limitado das escolhas reais no cenário FIFO cheia + IRQ.
// Escrita e leitura apenas em syscalls/IRQs IF=0, CPU única e feature opt-in.
#[cfg(feature = "teste-irq-fifo-cheia")]
static mut TESTE_IRQ_FIFO_TRACE_ATIVO: bool = false;
#[cfg(feature = "teste-irq-fifo-cheia")]
static mut TESTE_IRQ_FIFO_TRACE_RESTANTE: u8 = 0;
// Instrumentação delimitada do ensaio de morte do proprietário de IRQ4.
// Acessada apenas no despacho de IRQ/syscall com IF=0, em CPU única.
#[cfg(feature = "teste-irq-morte")]
static mut TESTE_IRQ_MORTE_AMOSTRAS: u8 = 0;
#[cfg(feature = "teste-irq-morte")]
static mut TESTE_IRQ_MORTE_BASE: u64 = 0;
#[cfg(feature = "teste-irq-morte")]
static mut TESTE_IRQ_MORTE_TIQUE_INICIAL: u64 = 0;
#[cfg(feature = "teste-irq-morte")]
static mut TESTE_IRQ_MORTE_TIQUE_FINAL: u64 = 0;
#[cfg(feature = "teste-irq-morte")]
static mut TESTE_IRQ_MORTE_MASCARA_OK: u8 = 0;
#[cfg(feature = "teste-irq-morte")]
static mut TESTE_IRQ_MORTE_ASSOCIACAO_OK: u8 = 0;
#[cfg(feature = "teste-irq-morte")]
static mut TESTE_IRQ_MORTE_IRQ4_ENTRADAS: u64 = 0;
#[cfg(feature = "teste-irq-morte")]
static mut TESTE_IRQ_MORTE_IRQ4_BASE: u64 = 0;
// Observacao temporaria do RZ022. O acesso ocorre em entradas de syscall/IRQ
// com IF=0; a referencia vincula o timeout exatamente ao registro observado.
#[cfg(feature = "teste-rz022")]
static mut TESTE_RZ022_REDE_BLOQUEADA: bool = false;
#[cfg(feature = "teste-rz022")]
static mut TESTE_RZ022_REDE_TIMEOUT: bool = false;
#[cfg(feature = "teste-rz022")]
static mut TESTE_RZ022_REDE_PENDENTE: bool = false;
#[cfg(feature = "teste-rz022")]
static mut TESTE_RZ022_REDE_REFERENCIA: u64 = 0;
#[cfg(feature = "teste-rz022")]
static mut TESTE_RZ022_REDE_ANUNCIO: bool = false;
// Fixture T03-05. Estado one-shot protegido pelo despacho IF=0/CPU única;
// guarda o prazo real da primeira espera, inclusive após ela expirar.
#[cfg(feature = "teste-prazo-empate")]
static mut TESTE_PRAZO_EMPATE_ARMADO: bool = false;
#[cfg(feature = "teste-prazo-empate")]
static mut TESTE_PRAZO_EMPATE_REGISTRADO: bool = false;
#[cfg(feature = "teste-prazo-empate")]
static mut TESTE_PRAZO_EMPATE_TIMEOUT: bool = false;
#[cfg(feature = "teste-prazo-empate")]
static mut TESTE_PRAZO_EMPATE_RECEBER_VISTO: bool = false;
#[cfg(feature = "teste-prazo-empate")]
static mut TESTE_PRAZO_EMPATE_SONDA_VISTA: bool = false;
#[cfg(feature = "teste-prazo-empate")]
static mut TESTE_PRAZO_EMPATE_SONDA_REGISTRADA_VISTA: bool = false;
#[cfg(feature = "teste-prazo-empate")]
static mut TESTE_PRAZO_EMPATE_REFERENCIA: u64 = 0;
#[cfg(feature = "teste-prazo-empate")]
static mut TESTE_PRAZO_EMPATE_PRAZO: u64 = 0;
#[cfg(feature = "teste-rx-quota")]
static mut TESTE_RX_QUOTA_FILA_CHEIA_VISTA: bool = false;
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
        estado: Estado::ATIVO,
        referencia,
        cr3: imagem.cr3,
        fisico: imagem.fisico,
        pilha,
        quadro,
        fila: CaixaMensagens::NOVA,
        espera_ipc: EsperaIpc::NOVA,
        recepcao_pendente: false,
        prazo_recepcao: None,
        alvo_observado: 0,
        observacao: Observacao::NOVA,
        espera_evento: false,
        preferir_vida: false,
        espera_irq: false,
        preferir_irq: false,
    }
}

pub unsafe fn executar() -> ! {
    ATUAL = 0;
    #[cfg(feature = "teste-prazo-empate")]
    crate::io::serial_texto("[teste-prazo-empate] KERNEL_FEATURE=1\n");
    let p = PROCESSOS[0];
    cpu::selecionar(p.pilha, true, p.cr3);
    #[cfg(feature = "teste-idle")]
    {
        for indice in 0..QUANTIDADE_SERVICOS {
            assert!(PROCESSOS[indice].estado.bloquear());
        }
        crate::io::serial_texto("[teste-idle] TODOS_BLOQUEADOS\n");
        cpu::executar(iniciar_ocioso());
    }
    #[cfg(not(feature = "teste-idle"))]
    cpu::executar(p.quadro)
}
unsafe fn agendar() -> *mut Quadro {
    for deslocamento in 1..=QUANTIDADE_SERVICOS {
        let indice = (ATUAL + deslocamento) % QUANTIDADE_SERVICOS;
        if !preparar_retorno(indice) {
            continue;
        }
        let p = PROCESSOS[indice];
        if p.estado.executavel() {
            OCIOSO = false;
            ATUAL = indice;
            cpu::selecionar(p.pilha, indice == 0, p.cr3);
            return p.quadro;
        }
    }
    iniciar_ocioso()
}
/// IF=0. A entrega ocorre somente ao escolher o processo; antes disso uma
/// revogacao ainda pode purgar a fila. Apos copiar, IRET retomara o servico:
/// uma IRQ antes da primeira instrucao nao desfaz a mensagem ja consumida.
unsafe fn preparar_retorno(indice: usize) -> bool {
    expirar_processo(indice);
    let atuais = referencias_atuais();
    let p = &mut *addr_of_mut!(PROCESSOS).cast::<Processo>().add(indice);
    if !p.estado.executavel() {
        return false;
    }
    if p.recepcao_pendente {
        if p.espera_irq {
            // O quadro reside na pilha de kernel da instancia, distinta da
            // tabela PROCESSOS que contem p; ambos permanecem exclusivos em IF=0.
            let quadro = p.quadro;
            if selecionar_irq_ipc(p, &atuais, &mut *quadro) {
                #[cfg(feature = "teste-uart")]
                if (*quadro).rax == classe_evento::IRQ {
                    crate::io::serial_texto("[teste-uart] DESPERTA_IRQ\n");
                }
                p.recepcao_pendente = false;
                p.prazo_recepcao = None;
                p.espera_irq = false;
                return true;
            }
            p.espera_ipc
                .registrar(&mut p.estado, 0)
                .expect("estado invalido ao rearmar IRQ");
            return false;
        }
        if p.espera_evento {
            if let Some(evento) = selecionar(
                &mut p.fila,
                &mut p.observacao,
                &mut p.preferir_vida,
                &atuais,
            ) {
                p.recepcao_pendente = false;
                p.prazo_recepcao = None;
                p.espera_evento = false;
                #[cfg(feature = "teste-rz022")]
                if indice as u64 + 1 == SUPERVISOR {
                    if let EventoEscolhido::Vida(vida) = evento {
                        let observador = p.referencia;
                        let tique = TIQUES;
                        let _ = writeln!(
                            crate::io::Emergencia,
                            "[teste-rz022] VIDA_DESPERTA observador={:#x} referencia={:#x} tipo={} estado={} tique={}",
                            observador,
                            vida.referencia,
                            vida.tipo,
                            vida.estado,
                            tique
                        );
                    }
                }
                preencher_evento(&mut *p.quadro, evento);
                return true;
            }
            p.espera_ipc
                .registrar(&mut p.estado, 0)
                .expect("estado invalido ao rearmar evento");
            return false;
        }
        if let Some(m) = preparar_recepcao(&mut p.fila, &mut p.espera_ipc, &mut p.estado, &atuais)
            .expect("estado invalido ao concluir recepcao")
        {
            #[cfg(feature = "teste-rz022")]
            if indice as u64 + 1 == REDE && TESTE_RZ022_REDE_REFERENCIA == p.referencia {
                TESTE_RZ022_REDE_PENDENTE = false;
                TESTE_RZ022_REDE_REFERENCIA = 0;
            }
            p.recepcao_pendente = false;
            p.prazo_recepcao = None;
            p.alvo_observado = 0;
            preencher_recebimento_bloqueante(&mut *p.quadro, m);
            #[cfg(all(feature = "teste-prazo", not(feature = "teste-ciclo-espera")))]
            let _ = writeln!(
                crate::io::Emergencia,
                "[teste-prazo] MENSAGEM id={}",
                indice + 1
            );
            #[cfg(any(feature = "teste-ipc-bloqueante", feature = "teste-receber-bloqueante"))]
            let _ = writeln!(
                crate::io::Emergencia,
                "[teste-ipc] RECEBER_CONCLUIDO id={}",
                indice + 1
            );
        } else {
            return false;
        }
    }
    true
}
/// IF=0. Varredura da instancia publicada, sem callbacks ou ponteiros antigos.
unsafe fn expirar_processo(indice: usize) {
    let p = &mut *addr_of_mut!(PROCESSOS).cast::<Processo>().add(indice);
    if expirar_recepcao(
        TIQUES,
        &mut p.prazo_recepcao,
        &mut p.recepcao_pendente,
        &mut p.espera_ipc,
        &mut p.estado,
    ) {
        (*p.quadro).rax = erro::TEMPO;
        #[cfg(feature = "teste-prazo-empate")]
        if indice == 0
            && TESTE_PRAZO_EMPATE_REGISTRADO
            && TESTE_PRAZO_EMPATE_REFERENCIA == p.referencia
            && !TESTE_PRAZO_EMPATE_TIMEOUT
        {
            TESTE_PRAZO_EMPATE_TIMEOUT = true;
            let referencia = TESTE_PRAZO_EMPATE_REFERENCIA;
            let prazo = TESTE_PRAZO_EMPATE_PRAZO;
            let tick = TIQUES;
            let _ = writeln!(
                crate::io::Emergencia,
                "[teste-prazo-empate] TIMEOUT referencia={:#x} tick={} prazo={}",
                referencia,
                tick,
                prazo
            );
        }
        #[cfg(feature = "teste-rz022")]
        if indice as u64 + 1 == REDE
            && TESTE_RZ022_REDE_PENDENTE
            && TESTE_RZ022_REDE_REFERENCIA == p.referencia
        {
            TESTE_RZ022_REDE_PENDENTE = false;
            TESTE_RZ022_REDE_REFERENCIA = 0;
            if !TESTE_RZ022_REDE_TIMEOUT {
                TESTE_RZ022_REDE_TIMEOUT = true;
                let referencia = p.referencia;
                let _ = writeln!(
                    crate::io::Emergencia,
                    "[teste-rz022] REDE_TIMEOUT referencia={:#x} prazo=10",
                    referencia
                );
            }
        }
        p.alvo_observado = 0;
        p.espera_evento = false;
        p.espera_irq = false;
        #[cfg(feature = "teste-ciclo-espera")]
        {
            if indice as u64 + 1 != TERMINAL {
                // IF=0: copia o contador antes de formatar, sem referencia a static mut.
                let tiques_observados = TIQUES;
                let _ = writeln!(
                    crate::io::Emergencia,
                    "[teste-ciclo-espera] TIMEOUT referencia={:#x} tiques={}",
                    p.referencia,
                    tiques_observados
                );
            }
        }
        #[cfg(all(feature = "teste-prazo", not(feature = "teste-ciclo-espera")))]
        let _ = writeln!(
            crate::io::Emergencia,
            "[teste-prazo] TIMEOUT id={}",
            indice + 1
        );
    }
}
unsafe fn cancelar_processo(indice: usize) {
    let p = &mut *addr_of_mut!(PROCESSOS).cast::<Processo>().add(indice);
    #[cfg(feature = "teste-ciclo-espera")]
    if p.recepcao_pendente {
        let _ = writeln!(
            crate::io::Emergencia,
            "[teste-ciclo-espera] CANCELADA referencia={:#x}",
            p.referencia
        );
    }
    cancelar_espera(
        &mut p.espera_ipc,
        &mut p.recepcao_pendente,
        &mut p.prazo_recepcao,
    );
    #[cfg(feature = "teste-rz022")]
    if indice as u64 + 1 == REDE && TESTE_RZ022_REDE_REFERENCIA == p.referencia {
        TESTE_RZ022_REDE_PENDENTE = false;
        TESTE_RZ022_REDE_REFERENCIA = 0;
    }
    p.alvo_observado = 0;
    p.espera_evento = false;
    p.espera_irq = false;
    p.observacao = Observacao::NOVA;
    if indice == 0 && IRQ4.referencia == p.referencia {
        #[cfg(feature = "teste-irq-morte")]
        {
            let sequencia = (&*addr_of_mut!(IRQ4)).sequencia_entregue();
            if sequencia != 0 {
                let mascara_antes = (crate::io::entrada8(0x21) & 0x10) != 0;
                let _ = writeln!(
                    crate::io::Emergencia,
                    "[teste-irq-morte] IRQ4_REVOGACAO_INICIO referencia={:#x} seq={} mascara={}",
                    p.referencia,
                    sequencia,
                    mascara_antes as u8
                );
                // 50 amostras no máximo, uma por IRQ0 após a falha; IF=0.
                let tique_base = TIQUES;
                TESTE_IRQ_MORTE_BASE = tique_base;
                TESTE_IRQ_MORTE_AMOSTRAS = 50;
                TESTE_IRQ_MORTE_IRQ4_BASE = TESTE_IRQ_MORTE_IRQ4_ENTRADAS;
                TESTE_IRQ_MORTE_TIQUE_INICIAL = 0;
                TESTE_IRQ_MORTE_TIQUE_FINAL = 0;
                TESTE_IRQ_MORTE_MASCARA_OK = 0;
                TESTE_IRQ_MORTE_ASSOCIACAO_OK = 0;
            }
        }
        cpu::mascara_irq4(true);
        (&mut *addr_of_mut!(IRQ4)).cancelar();
        #[cfg(feature = "teste-irq-morte")]
        if TESTE_IRQ_MORTE_AMOSTRAS != 0 {
            let referencia_cancelada = IRQ4.referencia;
            let mascara_depois = (crate::io::entrada8(0x21) & 0x10) != 0;
            let tique = TIQUES;
            let _ = writeln!(
                crate::io::Emergencia,
                "[teste-irq-morte] IRQ4_REVOGADA referencia={} mascara={} tique={}",
                referencia_cancelada,
                mascara_depois as u8,
                tique
            );
        }
    }
}

/// Em IF=0 no tratamento de IRQ0: coleta no máximo 50 observações após a falha.
#[cfg(feature = "teste-irq-morte")]
unsafe fn amostrar_timer_apos_morte() {
    if TESTE_IRQ_MORTE_AMOSTRAS == 0 {
        return;
    }
    let mascara = (crate::io::entrada8(0x21) & 0x10) != 0;
    let referencia = IRQ4.referencia;
    if TESTE_IRQ_MORTE_AMOSTRAS == 50 {
        TESTE_IRQ_MORTE_TIQUE_INICIAL = TIQUES;
    }
    TESTE_IRQ_MORTE_TIQUE_FINAL = TIQUES;
    TESTE_IRQ_MORTE_MASCARA_OK += mascara as u8;
    TESTE_IRQ_MORTE_ASSOCIACAO_OK += (referencia == 0) as u8;
    TESTE_IRQ_MORTE_AMOSTRAS -= 1;
    if TESTE_IRQ_MORTE_AMOSTRAS == 0 {
        let decorrido = TIQUES.wrapping_sub(TESTE_IRQ_MORTE_BASE);
        let entradas_irq4 = TESTE_IRQ_MORTE_IRQ4_ENTRADAS.wrapping_sub(TESTE_IRQ_MORTE_IRQ4_BASE);
        let tique_inicial = TESTE_IRQ_MORTE_TIQUE_INICIAL;
        let tique_final = TESTE_IRQ_MORTE_TIQUE_FINAL;
        let mascara_ok = TESTE_IRQ_MORTE_MASCARA_OK;
        let associacao_ausente_ok = TESTE_IRQ_MORTE_ASSOCIACAO_OK;
        let _ = writeln!(
            crate::io::Emergencia,
            "[teste-irq-morte] TIMER_AMOSTRAS quantidade=50 tique_inicial={} tique_final={} desde_falha={} mascara_ok={} associacao_ausente_ok={} entradas_irq4={}",
            tique_inicial,
            tique_final,
            decorrido,
            mascara_ok,
            associacao_ausente_ok,
            entradas_irq4
        );
    }
}

/// IF=0: selecao justa entre FIFO e slot IRQ, com prazo tratado pelo chamador.
unsafe fn selecionar_irq_ipc(
    p: &mut Processo,
    atuais: &[u64; QUANTIDADE_SERVICOS],
    f: &mut Quadro,
) -> bool {
    #[cfg(feature = "teste-seletor-irq-ipc")]
    let observacao = if TESTE_IRQ_IPC_TRACE_ATIVO && p.referencia == atuais[0] {
        let irq_pendente = (&*addr_of_mut!(IRQ4)).selecionavel(p.referencia);
        let ipc_validas = p.fila.contar_atual(atuais);
        (irq_pendente && ipc_validas > 0).then_some((p.preferir_irq, ipc_validas, p.referencia))
    } else {
        None
    };
    #[cfg(feature = "teste-irq-fifo-cheia")]
    let observacao_fifo_cheia = if TESTE_IRQ_FIFO_TRACE_ATIVO && p.referencia == atuais[0] {
        Some((
            p.preferir_irq,
            p.fila.tamanho(),
            p.fila.contar_atual(atuais),
            (&*addr_of_mut!(IRQ4)).selecionavel(p.referencia),
            p.referencia,
        ))
    } else {
        None
    };
    for irq in [p.preferir_irq, !p.preferir_irq] {
        if irq {
            match (&mut *addr_of_mut!(IRQ4)).retirar() {
                Ok(Some(sequencia)) => {
                    f.rax = classe_evento::IRQ;
                    f.rdi = sequencia;
                    p.preferir_irq = false;
                    #[cfg(feature = "teste-seletor-irq-ipc")]
                    if let Some((preferir_irq, ipc_validas, terminal_ref)) = observacao {
                        let _ = writeln!(
                            crate::io::Emergencia,
                            "[teste-seletor-irq-ipc] SELETOR terminal_ref={:#x} preferir_irq={} irq_pendente=1 irq_ref_atual=1 entregue=0 ipc_validas={} escolhido=IRQ seq={}",
                            terminal_ref,
                            preferir_irq as u8,
                            ipc_validas,
                            sequencia
                        );
                        TESTE_IRQ_IPC_TRACE_RESTANTE =
                            TESTE_IRQ_IPC_TRACE_RESTANTE.saturating_sub(1);
                        TESTE_IRQ_IPC_TRACE_ATIVO = TESTE_IRQ_IPC_TRACE_RESTANTE != 0;
                    }
                    #[cfg(feature = "teste-irq-fifo-cheia")]
                    if let Some((preferir_irq, tamanho, ipc_validas, pendente, terminal_ref)) =
                        observacao_fifo_cheia
                    {
                        let _ = writeln!(
                            crate::io::Emergencia,
                            "[teste-irq-fifo-cheia] SELETOR terminal_ref={:#x} preferir_irq={} ipc_validas={} fila={} irq_pendente={} escolhido=IRQ seq={}",
                            terminal_ref,
                            preferir_irq as u8,
                            ipc_validas,
                            tamanho,
                            pendente as u8,
                            sequencia
                        );
                        TESTE_IRQ_FIFO_TRACE_RESTANTE =
                            TESTE_IRQ_FIFO_TRACE_RESTANTE.saturating_sub(1);
                        TESTE_IRQ_FIFO_TRACE_ATIVO = TESTE_IRQ_FIFO_TRACE_RESTANTE != 0;
                    }
                    return true;
                }
                Err(e) => {
                    f.rax = e;
                    return true;
                }
                Ok(None) => {}
            }
        } else if let Some(m) = retirar_atual(&mut p.fila, atuais) {
            preencher_evento(f, EventoEscolhido::Ipc(m));
            p.preferir_irq = true;
            #[cfg(feature = "teste-seletor-irq-ipc")]
            if let Some((preferir_irq, ipc_validas, terminal_ref)) = observacao {
                let _ = writeln!(
                    crate::io::Emergencia,
                    "[teste-seletor-irq-ipc] SELETOR terminal_ref={:#x} preferir_irq={} irq_pendente=1 irq_ref_atual=1 entregue=0 ipc_validas={} escolhido=IPC origem={:#x} origem_atual=1 codigo={:#x} a={}",
                    terminal_ref,
                    preferir_irq as u8,
                    ipc_validas,
                    m.origem,
                    m.codigo,
                    m.a
                );
                TESTE_IRQ_IPC_TRACE_RESTANTE = TESTE_IRQ_IPC_TRACE_RESTANTE.saturating_sub(1);
                TESTE_IRQ_IPC_TRACE_ATIVO = TESTE_IRQ_IPC_TRACE_RESTANTE != 0;
            }
            #[cfg(feature = "teste-irq-fifo-cheia")]
            if let Some((preferir_irq, tamanho, ipc_validas, pendente, terminal_ref)) =
                observacao_fifo_cheia
            {
                let _ = writeln!(
                    crate::io::Emergencia,
                    "[teste-irq-fifo-cheia] SELETOR terminal_ref={:#x} preferir_irq={} ipc_validas={} fila={} irq_pendente={} escolhido=IPC origem={:#x} codigo={:#x} a={}",
                    terminal_ref,
                    preferir_irq as u8,
                    ipc_validas,
                    tamanho,
                    pendente as u8,
                    m.origem,
                    m.codigo,
                    m.a
                );
                TESTE_IRQ_FIFO_TRACE_RESTANTE = TESTE_IRQ_FIFO_TRACE_RESTANTE.saturating_sub(1);
                TESTE_IRQ_FIFO_TRACE_ATIVO = TESTE_IRQ_FIFO_TRACE_RESTANTE != 0;
            }
            return true;
        }
    }
    false
}
unsafe fn registrar_irq4() {
    #[cfg(feature = "teste-irq-morte")]
    if TESTE_IRQ_MORTE_AMOSTRAS != 0 {
        TESTE_IRQ_MORTE_IRQ4_ENTRADAS = TESTE_IRQ_MORTE_IRQ4_ENTRADAS.saturating_add(1);
    }
    #[cfg(feature = "teste-irq")]
    {
        TESTE_IRQ4_ENTRADAS = TESTE_IRQ4_ENTRADAS.saturating_add(1);
    }
    cpu::mascara_irq4(true);
    (&mut *addr_of_mut!(IRQ4)).publicar();
    cpu::confirmar_irq0(); // EOI do PIC mestre, nao reconhece a UART.
    expirar_processo(0);
    let p = &mut PROCESSOS[0];
    if p.espera_irq && p.recepcao_pendente && IRQ4.referencia == p.referencia {
        p.espera_ipc.mensagem_admitida(&mut p.estado);
    }
}
/// IF=0. Prazo vencido precede morte; IPC nao consumido permanece na FIFO.
unsafe fn invalidar_observadores(alvo: u64) {
    for indice in 0..QUANTIDADE_SERVICOS {
        expirar_processo(indice);
        let p = &mut *addr_of_mut!(PROCESSOS).cast::<Processo>().add(indice);
        if invalidar_espera_por_alvo(
            alvo,
            &mut p.alvo_observado,
            &mut p.espera_ipc,
            &mut p.recepcao_pendente,
            &mut p.prazo_recepcao,
            &mut p.estado,
        ) {
            (*p.quadro).rax = erro::ALVO_INDISPONIVEL;
            #[cfg(feature = "teste-ciclo-espera")]
            let _ = writeln!(
                crate::io::Emergencia,
                "[teste-ciclo-espera] ALVO_INDISPONIVEL referencia={:#x}",
                alvo
            );
        }
    }
}
/// Publica em slot reservado; nunca consome a FIFO IPC. IF=0.
unsafe fn publicar_vida(alvo: u64, estado: u64, invalidacao: bool) {
    for indice in 0..QUANTIDADE_SERVICOS {
        expirar_processo(indice);
        let p = &mut *addr_of_mut!(PROCESSOS).cast::<Processo>().add(indice);
        let publicado = if invalidacao {
            p.observacao.invalidar(alvo)
        } else {
            p.observacao.publicar_estado(alvo, estado)
        };
        if publicado && p.espera_evento && p.recepcao_pendente {
            p.espera_ipc.mensagem_admitida(&mut p.estado);
        }
        #[cfg(feature = "teste-vida")]
        if publicado {
            let _ = writeln!(
                crate::io::Emergencia,
                "[teste-vida] PUBLICADO referencia={:#x} invalidacao={}",
                alvo,
                invalidacao as u8
            );
        }
    }
}
unsafe fn expirar_todos() {
    for indice in 0..QUANTIDADE_SERVICOS {
        expirar_processo(indice);
    }
}
/// IF=0. A pilha dedicada so e reinicializada ao entrar vindo de um servico.
unsafe fn iniciar_ocioso() -> *mut Quadro {
    let topo = addr_of_mut!(PILHA_OCIOSA) as u64 + PILHA_KERNEL as u64;
    let quadro = (topo - mem::size_of::<Quadro>() as u64) as *mut Quadro;
    write(
        quadro,
        Quadro {
            rip: ocioso as *const () as u64,
            cs: 0x08,
            rflags: 0x202,
            rsp: topo - 8,
            ss: 0x10,
            ..Quadro::default()
        },
    );
    #[cfg(feature = "teste-rz024")]
    if !OCIOSO && TESTE_RZ024_ARMADO && TESTE_RZ024_ENTRADAS_IDLE == 0 {
        TESTE_RZ024_ENTRADAS_IDLE = TESTE_RZ024_ENTRADAS_IDLE.saturating_add(1);
        TESTE_RZ024_TIQUES_IDLE = 0;
        TESTE_RZ024_IRQ4_IDLE = 0;
        TESTE_RZ024_REFERENCIAS = referencias_atuais();
        TESTE_RZ024_EPISODIO_ATIVO = true;
        let refs = TESTE_RZ024_REFERENCIAS;
        let cenario = if cfg!(feature = "teste-irq") {
            "irq4"
        } else if cfg!(feature = "teste-prazo-idle") {
            "prazo"
        } else {
            "pressao-finita"
        };
        let entradas = TESTE_RZ024_ENTRADAS_IDLE;
        let _ = writeln!(
            crate::io::Emergencia,
            "[teste-rz024] IDLE_INICIO cenario={} janela=1 entradas_idle={} executaveis=0 terminal={:#x} rede={:#x} supervisor={:#x}",
            cenario, entradas, refs[0], refs[1], refs[2]
        );
    }
    OCIOSO = true;
    #[cfg(feature = "teste-irq")]
    crate::io::serial_texto("[teste-irq] OCIOSO\n");
    quadro
}
/// A IRQ0 verifica novamente processos prontos com IF=0; IRET restaura IF.
extern "C" fn ocioso() -> ! {
    loop {
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack));
        }
    }
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
        Ok(indice) => PROCESSOS[indice].estado.abi(),
        Err(e) => e,
    }
}

#[no_mangle]
pub unsafe extern "C" fn despachar_interrupcao(q: *mut Quadro) -> *mut Quadro {
    let f = &mut *q;
    if f.cs & 3 == 0 && OCIOSO && (f.vetor == 32 || f.vetor == 36) {
        #[cfg(feature = "teste-rz024")]
        if TESTE_RZ024_EPISODIO_ATIVO {
            if f.vetor == 32 {
                TESTE_RZ024_TIQUES_IDLE = TESTE_RZ024_TIQUES_IDLE.saturating_add(1);
            } else {
                TESTE_RZ024_IRQ4_IDLE = TESTE_RZ024_IRQ4_IDLE.saturating_add(1);
            }
        }
        if f.vetor == 36 {
            #[cfg(feature = "teste-irq")]
            crate::io::serial_texto("[teste-irq] IRQ4_CPL0\n");
            registrar_irq4();
        } else {
            TIQUES = TIQUES.wrapping_add(1);
            cpu::confirmar_irq0();
            #[cfg(feature = "teste-irq-morte")]
            amostrar_timer_apos_morte();
            #[cfg(feature = "teste-prazo-idle")]
            if !TESTE_PRAZO_IDLE_IRQ_REGISTRADA {
                TESTE_PRAZO_IDLE_IRQ_REGISTRADA = true;
                crate::io::serial_texto("[teste-prazo-idle] IRQ0_CPL0\n");
            }
            expirar_todos();
        }
        #[cfg(feature = "teste-idle")]
        if TIQUES == 3 {
            crate::io::serial_texto("[teste-idle] IRQ0_CPL0\n");
            for indice in 0..QUANTIDADE_SERVICOS {
                assert!(PROCESSOS[indice].estado.despertar());
            }
            crate::io::serial_texto("[teste-idle] SERVICOS_PRONTOS\n");
        }
        // Sem candidato, preservar exatamente o frame interrompido do idle.
        for deslocamento in 1..=QUANTIDADE_SERVICOS {
            let indice = (ATUAL + deslocamento) % QUANTIDADE_SERVICOS;
            if !preparar_retorno(indice) {
                continue;
            }
            let p = PROCESSOS[indice];
            if p.estado.executavel() {
                #[cfg(feature = "teste-rz024")]
                if TESTE_RZ024_EPISODIO_ATIVO {
                    // O kernel observou IRQ0; somente a fixture/harness pode
                    // correlacionar depois esse tique ao prazo que expirou.
                    let fonte = if f.vetor == 36 { "IRQ4" } else { "IRQ0" };
                    let cenario = if cfg!(feature = "teste-irq") {
                        "irq4"
                    } else if cfg!(feature = "teste-prazo-idle") {
                        "prazo"
                    } else {
                        "pressao-finita"
                    };
                    let mascara = (crate::io::entrada8(0x21) & 0x10) >> 4;
                    let entradas = TESTE_RZ024_ENTRADAS_IDLE;
                    let tiques = TESTE_RZ024_TIQUES_IDLE;
                    let irq4 = TESTE_RZ024_IRQ4_IDLE;
                    let _ = writeln!(
                        crate::io::Emergencia,
                        "[teste-rz024] IDLE_FIM cenario={} janela=1 fonte={} entradas_idle={} tiques_irq0_idle={} entradas_irq4_idle={} mascara_irq4={} selecionado={:#x}",
                        cenario,
                        fonte,
                        entradas,
                        tiques,
                        irq4,
                        mascara,
                        p.referencia
                    );
                    TESTE_RZ024_EPISODIO_ATIVO = false;
                }
                OCIOSO = false;
                ATUAL = indice;
                cpu::selecionar(p.pilha, indice == 0, p.cr3);
                return p.quadro;
            }
        }
        return q;
    }
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
    #[cfg(feature = "teste-idle")]
    if !TESTE_RETORNO_REGISTRADO {
        TESTE_RETORNO_REGISTRADO = true;
        crate::io::serial_texto("[teste-idle] RETORNO_CPL3\n");
    }
    PROCESSOS[ATUAL].quadro = q;
    match f.vetor {
        36 => {
            registrar_irq4();
            agendar()
        }
        32 => {
            TIQUES = TIQUES.wrapping_add(1);
            cpu::confirmar_irq0();
            #[cfg(feature = "teste-irq-morte")]
            amostrar_timer_apos_morte();
            expirar_todos();
            agendar()
        }
        128 => {
            match f.rax {
                // Sonda privada e read-only T03-05. Só a rede em CPL3 pode
                // consultar o prazo registrado para o terminal. A entrada
                // int 0x80 mantém IF=0; nenhum IPC ou estado é consumido.
                #[cfg(feature = "teste-prazo-empate")]
                0xff06 => {
                    if !TESTE_PRAZO_EMPATE_SONDA_VISTA {
                        TESTE_PRAZO_EMPATE_SONDA_VISTA = true;
                        let chamador = ATUAL as u64 + 1;
                        let _ = writeln!(
                            crate::io::Emergencia,
                            "[teste-prazo-empate] SONDA_CHAMADA chamador={} cpl={} alvo={:#x} armada={} registrada={}",
                            chamador,
                            f.cs & 3,
                            f.rdi,
                            TESTE_PRAZO_EMPATE_ARMADO as u8,
                            TESTE_PRAZO_EMPATE_REGISTRADO as u8
                        );
                    }
                    if ATUAL as u64 + 1 != REDE || f.cs & 3 != 3 {
                        f.rax = erro::PERMISSAO;
                    } else if f.rdi != PROCESSOS[0].referencia {
                        f.rax = erro::REFERENCIA_ANTIGA;
                    } else if TESTE_PRAZO_EMPATE_REGISTRADO
                        && TESTE_PRAZO_EMPATE_REFERENCIA == PROCESSOS[0].referencia
                    {
                        if !TESTE_PRAZO_EMPATE_SONDA_REGISTRADA_VISTA {
                            TESTE_PRAZO_EMPATE_SONDA_REGISTRADA_VISTA = true;
                            let referencia = TESTE_PRAZO_EMPATE_REFERENCIA;
                            let prazo = TESTE_PRAZO_EMPATE_PRAZO;
                            let _ = writeln!(
                                crate::io::Emergencia,
                                "[teste-prazo-empate] SONDA_REGISTRADA referencia={:#x} prazo={} timeout={}",
                                referencia,
                                prazo,
                                TESTE_PRAZO_EMPATE_TIMEOUT as u8
                            );
                        }
                        f.rax = 1;
                        f.rdi = TESTE_PRAZO_EMPATE_PRAZO;
                        f.rsi = TESTE_PRAZO_EMPATE_REFERENCIA;
                        f.rdx = TESTE_PRAZO_EMPATE_TIMEOUT as u64;
                    } else {
                        f.rax = 0;
                        f.rdi = 0;
                        f.rsi = PROCESSOS[0].referencia;
                        f.rdx = TESTE_PRAZO_EMPATE_TIMEOUT as u64;
                    }
                }
                #[cfg(feature = "teste-irq")]
                0xff00 => {
                    if ATUAL != 0 {
                        f.rax = erro::PERMISSAO;
                    } else {
                        f.rax = TESTE_IRQ4_ENTRADAS;
                        // Leitura do IMR nao altera a mascara nem reconhece IRQ.
                        f.rdi = (crate::io::entrada8(0x21) & 0x10) as u64;
                    }
                }
                // Sondagem privada do ensaio: ATUAL e o chamador devem ser o
                // terminal. IF=0 serializa a leitura; nenhum evento e retirado.
                #[cfg(feature = "teste-seletor-irq-ipc")]
                0xff01 => {
                    if ATUAL != 0 || f.cs & 3 != 3 {
                        f.rax = erro::PERMISSAO;
                    } else {
                        let atuais = referencias_atuais();
                        let terminal = PROCESSOS[0];
                        let irq_pronto = (&*addr_of_mut!(IRQ4)).selecionavel(terminal.referencia);
                        let pronto = irq_pronto
                            && terminal
                                .fila
                                .prefixo_atual_confere(&atuais, atuais[1], f.rdi, f.rsi, f.rdx);
                        f.rax = pronto as u64;
                        f.rdi = terminal.fila.contar_atual(&atuais) as u64;
                        f.rsi = terminal.preferir_irq as u64;
                        f.rdx = irq_pronto as u64;
                        // IRQ4 fica mascarada entre publicar e confirmar. Leitura
                        // de IMR apenas observa; nao altera PIC nem reconhece IRQ.
                        f.r10 = (crate::io::entrada8(0x21) & 0x10 != 0) as u64;
                        if pronto {
                            // Habilita apenas os dois registros seguintes do
                            // seletor real; filas, slot IRQ e preferencia intactos.
                            TESTE_IRQ_IPC_TRACE_RESTANTE = 2;
                            TESTE_IRQ_IPC_TRACE_ATIVO = true;
                        }
                    }
                }
                // Sonda read-only privada: terminal CPL3, IF=0. Confere a
                // composição da FIFO e o slot IRQ sem retirar mensagens/eventos.
                #[cfg(feature = "teste-irq-fifo-cheia")]
                0xff05 => {
                    if ATUAL != 0 || f.cs & 3 != 3 {
                        f.rax = erro::PERMISSAO;
                    } else {
                        let atuais = referencias_atuais();
                        let terminal = PROCESSOS[0];
                        let modo = f.rdi;
                        let (quantidade_rede, supervisor_presente, mascara_esperada, irq_esperada) =
                            match modo {
                                1 => (15, false, false, false),
                                2 => (15, true, false, false),
                                3 => (15, true, true, true),
                                4 => (0, false, false, false),
                                _ => (0, false, false, false),
                            };
                        let irq = &*addr_of_mut!(IRQ4);
                        let mascara = crate::io::entrada8(0x21) & 0x10 != 0;
                        let fila_ok = if modo == 4 {
                            terminal.fila.tamanho() == 0
                        } else {
                            quantidade_rede != 0
                                && terminal.fila.fila_irq_teste_confere(
                                    atuais[1],
                                    atuais[2],
                                    quantidade_rede,
                                    supervisor_presente,
                                )
                        };
                        let irq_ok = if irq_esperada {
                            irq.selecionavel(terminal.referencia)
                        } else {
                            irq.vazia_armada(terminal.referencia)
                        };
                        let pronto = fila_ok
                            && irq_ok
                            && mascara == mascara_esperada
                            && atuais[0] == terminal.referencia
                            && (modo != 4
                                || (!TESTE_IRQ_FIFO_TRACE_ATIVO
                                    && TESTE_IRQ_FIFO_TRACE_RESTANTE == 0));
                        f.rax = pronto as u64;
                        f.rsi = terminal.fila.tamanho() as u64;
                        f.rdx = terminal.preferir_irq as u64;
                        f.r10 = irq_ok as u64;
                        f.r8 = mascara as u64;
                        if modo == 3 && pronto {
                            TESTE_IRQ_FIFO_TRACE_ATIVO = true;
                            TESTE_IRQ_FIFO_TRACE_RESTANTE = 17;
                        }
                    }
                }
                // Sonda read-only opt-in: confirma que o slot IRQ4 não contém
                // evento pendente/entregue antes de armar o estímulo medido.
                #[cfg(feature = "teste-irq-morte")]
                0xff02 => {
                    if ATUAL != 0 || f.cs & 3 != 3 {
                        f.rax = erro::PERMISSAO;
                    } else {
                        f.rax = (&*addr_of_mut!(IRQ4)).ocupada(PROCESSOS[0].referencia) as u64;
                    }
                }
                // Sinal privado enviado pelo supervisor somente depois de a
                // consulta de estado ter retornado FALHOU ao código CPL3.
                #[cfg(feature = "teste-irq-morte")]
                0xff03 => {
                    if ATUAL as u64 + 1 != SUPERVISOR || f.cs & 3 != 3 {
                        f.rax = erro::PERMISSAO;
                    } else if f.rdi != PROCESSOS[0].referencia
                        || PROCESSOS[0].estado.abi() != contrato_sistema::estado::FALHOU
                    {
                        f.rax = erro::ESTADO;
                    } else {
                        f.rax = 0;
                        let _ = writeln!(
                            crate::io::Emergencia,
                            "[teste-irq-morte] SUPERVISOR_REAGIU_FALHOU referencia={:#x}",
                            f.rdi
                        );
                    }
                }
                // Arma uma janela de observação privada do RZ-024. O chamado
                // é somente do terminal CPL3; não muda estado de agendamento.
                #[cfg(feature = "teste-rz024")]
                0xff04 => {
                    let cenario = if cfg!(feature = "teste-irq") {
                        1
                    } else if cfg!(feature = "teste-prazo-idle") {
                        2
                    } else {
                        3
                    };
                    if ATUAL != 0 || f.cs & 3 != 3 {
                        f.rax = erro::PERMISSAO;
                    } else if f.rdi != cenario || TESTE_RZ024_ARMADO {
                        f.rax = erro::ESTADO;
                    } else {
                        TESTE_RZ024_ARMADO = true;
                        f.rax = 0;
                        let _ = writeln!(
                            crate::io::Emergencia,
                            "[teste-rz024] JANELA_ARMADA cenario={} referencia={:#x}",
                            if cenario == 1 { "irq4" } else { "prazo" },
                            PROCESSOS[0].referencia
                        );
                    }
                }
                // Sonda read-only T03-12. Terminal CPL3; int 0x80 entra com
                // IF=0 e a FIFO circular é examinada sem retirar mensagens.
                #[cfg(feature = "teste-rx-quota")]
                0xff07 => {
                    if ATUAL != 0 || f.cs & 3 != 3 {
                        f.rax = erro::PERMISSAO;
                    } else if !matches!(f.rdi, 1 | 2) {
                        f.rax = erro::ESTADO;
                    } else {
                        let atuais = referencias_atuais();
                        let quantidade = PROCESSOS[0].fila.tamanho();
                        let pronto = atuais[0] == PROCESSOS[0].referencia
                            && id_referencia(atuais[1]) == REDE
                            && id_referencia(atuais[2]) == SUPERVISOR
                            && PROCESSOS[0]
                                .fila
                                .rx_quota_confere(atuais[1], atuais[2], f.rdi == 2);
                        f.rax = pronto as u64;
                        f.rdi = quantidade as u64;
                    }
                }
                chamada::ARMAR_IRQ4 => {
                    if ATUAL != 0 {
                        f.rax = erro::PERMISSAO;
                    } else {
                        f.rax = match (&mut *addr_of_mut!(IRQ4)).armar(PROCESSOS[0].referencia) {
                            Ok(()) => {
                                cpu::mascara_irq4(false);
                                0
                            }
                            Err(e) => e,
                        };
                    }
                }
                chamada::CONFIRMAR_IRQ4 => {
                    if ATUAL != 0 {
                        f.rax = erro::PERMISSAO;
                    } else {
                        f.rax = match (&mut *addr_of_mut!(IRQ4))
                            .confirmar(PROCESSOS[0].referencia, f.rdi)
                        {
                            Ok(()) => {
                                cpu::mascara_irq4(false);
                                0
                            }
                            Err(e) => e,
                        };
                    }
                }
                chamada::ESPERAR_IRQ_IPC => {
                    if ATUAL != 0 {
                        f.rax = erro::PERMISSAO;
                        return q;
                    }
                    let modo = f.rdi;
                    let limite = match calcular_prazo(modo, f.rsi, TIQUES) {
                        Ok(v) => v,
                        Err(e) => {
                            f.rax = e;
                            return q;
                        }
                    };
                    let atuais = referencias_atuais();
                    let p = &mut PROCESSOS[0];
                    if IRQ4.referencia != p.referencia {
                        f.rax = erro::ESTADO;
                    } else if selecionar_irq_ipc(p, &atuais, f) {
                    } else if modo == prazo::IMEDIATO {
                        f.rax = erro::AUSENTE;
                    } else {
                        match p.espera_ipc.registrar(&mut p.estado, 0) {
                            Ok(true) => {
                                p.recepcao_pendente = true;
                                p.espera_irq = true;
                                p.prazo_recepcao = limite;
                                #[cfg(feature = "teste-irq")]
                                crate::io::serial_texto("[teste-irq] BLOQUEADO\n");
                                #[cfg(feature = "teste-uart")]
                                if !TESTE_UART_BLOQUEIO_REGISTRADO {
                                    TESTE_UART_BLOQUEIO_REGISTRADO = true;
                                    crate::io::serial_texto("[teste-uart] BLOQUEADO\n");
                                }
                                return agendar();
                            }
                            Err(e) => f.rax = e,
                            Ok(false) => f.rax = erro::ESTADO,
                        }
                    }
                }
                chamada::CEDER => {
                    f.rax = 0;
                    return agendar();
                }
                chamada::ENVIAR => f.rax = enviar(f),
                chamada::RECEBER => receber(f),
                chamada::INSCREVER_VIDA => {
                    let indice = match autorizar_gerenciamento(
                        ATUAL as u64 + 1,
                        f.rdi,
                        &referencias_atuais(),
                    ) {
                        Ok(i) => i,
                        Err(e) => {
                            f.rax = e;
                            return q;
                        }
                    };
                    let estado = PROCESSOS[indice].estado.abi();
                    let p = &mut *addr_of_mut!(PROCESSOS).cast::<Processo>().add(ATUAL);
                    match p.observacao.inscrever(f.rdi, estado) {
                        Ok(inicial) => {
                            f.rax = 0;
                            f.rdi = inicial;
                        }
                        Err(e) => f.rax = e,
                    }
                }
                chamada::CANCELAR_VIDA => {
                    if ATUAL as u64 + 1 != SUPERVISOR {
                        f.rax = erro::PERMISSAO;
                        return q;
                    }
                    if !referencia_valida(f.rdi) {
                        f.rax = erro::SERVICO;
                        return q;
                    }
                    if id_referencia(f.rdi) != REDE {
                        f.rax = erro::PERMISSAO;
                        return q;
                    }
                    let p = &mut *addr_of_mut!(PROCESSOS).cast::<Processo>().add(ATUAL);
                    f.rax = match p.observacao.cancelar(f.rdi) {
                        Ok(()) => 0,
                        Err(e) => e,
                    };
                }
                chamada::ESPERAR_EVENTO => {
                    let modo = f.rdi;
                    let limite = match calcular_prazo(modo, f.rsi, TIQUES) {
                        Ok(v) => v,
                        Err(e) => {
                            f.rax = e;
                            return q;
                        }
                    };
                    let atuais = referencias_atuais();
                    let p = &mut *addr_of_mut!(PROCESSOS).cast::<Processo>().add(ATUAL);
                    if !p.observacao.inscrita() {
                        f.rax = erro::ESTADO;
                        return q;
                    }
                    if let Some(evento) = selecionar(
                        &mut p.fila,
                        &mut p.observacao,
                        &mut p.preferir_vida,
                        &atuais,
                    ) {
                        preencher_evento(f, evento);
                    } else if modo == prazo::IMEDIATO {
                        f.rax = erro::AUSENTE;
                    } else {
                        match p.espera_ipc.registrar(&mut p.estado, 0) {
                            Ok(true) => {
                                p.recepcao_pendente = true;
                                p.espera_evento = true;
                                p.prazo_recepcao = limite;
                                #[cfg(feature = "teste-rz022")]
                                if ATUAL as u64 + 1 == SUPERVISOR
                                    && (modo == prazo::SEM_PRAZO || modo == prazo::FINITO)
                                {
                                    let observador = p.referencia;
                                    let referencia = atuais[(REDE - 1) as usize];
                                    let inicio = TIQUES;
                                    if modo == prazo::SEM_PRAZO {
                                        let _ = writeln!(
                                            crate::io::Emergencia,
                                            "[teste-rz022] EVENTO_BLOQUEADO observador={:#x} referencia={:#x} modo=SEM_PRAZO duracao={} inicio={}",
                                            observador,
                                            referencia,
                                            f.rsi,
                                            inicio
                                        );
                                    } else {
                                        let _ = writeln!(
                                            crate::io::Emergencia,
                                            "[teste-rz022] EVENTO_BLOQUEADO observador={:#x} referencia={:#x} modo=FINITO duracao={} inicio={}",
                                            observador,
                                            referencia,
                                            f.rsi,
                                            inicio
                                        );
                                    }
                                }
                                #[cfg(feature = "teste-vida")]
                                crate::io::serial_texto("[teste-vida] BLOQUEADO\n");
                                return agendar();
                            }
                            Err(e) => f.rax = e,
                            Ok(false) => f.rax = erro::ESTADO,
                        }
                    }
                }
                chamada::RECEBER_OBSERVANDO => {
                    let alvo = f.rdi;
                    let modo = f.rsi;
                    let indice = match autorizar_gerenciamento(
                        ATUAL as u64 + 1,
                        alvo,
                        &referencias_atuais(),
                    ) {
                        Ok(i) => i,
                        Err(e) => {
                            f.rax = e;
                            return q;
                        }
                    };
                    let prazo = match calcular_prazo(modo, f.rdx, TIQUES) {
                        Ok(p) => p,
                        Err(e) => {
                            f.rax = e;
                            return q;
                        }
                    };
                    if !PROCESSOS[indice].estado.ativo() {
                        f.rax = erro::ALVO_INDISPONIVEL;
                        return q;
                    }
                    let atuais = referencias_atuais();
                    let p = &mut *addr_of_mut!(PROCESSOS).cast::<Processo>().add(ATUAL);
                    if let Some(m) = retirar_atual(&mut p.fila, &atuais) {
                        preencher_recebimento_bloqueante(f, m);
                    } else if modo == prazo::IMEDIATO {
                        f.rax = erro::AUSENTE;
                    } else {
                        match p.espera_ipc.registrar(&mut p.estado, 0) {
                            Ok(true) => {
                                p.recepcao_pendente = true;
                                p.prazo_recepcao = prazo;
                                p.alvo_observado = alvo;
                                #[cfg(feature = "teste-ciclo-espera")]
                                let _ = writeln!(
                                    crate::io::Emergencia,
                                    "[teste-ciclo-espera] OBSERVANDO referencia={:#x}",
                                    alvo
                                );
                                return agendar();
                            }
                            Err(e) => f.rax = e,
                            Ok(false) => f.rax = erro::ESTADO,
                        }
                    }
                }
                chamada::RECEBER_COM_PRAZO => {
                    let modo = f.rdi;
                    #[cfg(feature = "teste-rz022")]
                    let rz022_anuncio = if ATUAL as u64 + 1 == REDE {
                        let anunciado = TESTE_RZ022_REDE_ANUNCIO;
                        TESTE_RZ022_REDE_ANUNCIO = false;
                        anunciado && modo == prazo::FINITO && f.rsi == 10
                    } else {
                        false
                    };
                    let prazo = match calcular_prazo(modo, f.rsi, TIQUES) {
                        Ok(p) => p,
                        Err(e) => {
                            f.rax = e;
                            return q;
                        }
                    };
                    let atuais = referencias_atuais();
                    let p = &mut *addr_of_mut!(PROCESSOS).cast::<Processo>().add(ATUAL);
                    #[cfg(feature = "teste-prazo-empate")]
                    if ATUAL == 0 && TESTE_PRAZO_EMPATE_ARMADO && !TESTE_PRAZO_EMPATE_RECEBER_VISTO
                    {
                        TESTE_PRAZO_EMPATE_RECEBER_VISTO = true;
                        let tick = TIQUES;
                        let fila = p.fila.tamanho();
                        let _ = writeln!(
                            crate::io::Emergencia,
                            "[teste-prazo-empate] RECEBER_CHAMADA modo={} duracao={} tick={} fila={}",
                            modo, f.rsi, tick, fila
                        );
                    }
                    if let Some(m) = retirar_atual(&mut p.fila, &atuais) {
                        #[cfg(feature = "teste-prazo-empate")]
                        if ATUAL == 0 && TESTE_PRAZO_EMPATE_ARMADO {
                            TESTE_PRAZO_EMPATE_ARMADO = false;
                            let _ = writeln!(
                                crate::io::Emergencia,
                                "[teste-prazo-empate] REGISTRO_RECUSADO motivo=mensagem_preexistente codigo={:#x}",
                                m.codigo
                            );
                        }
                        preencher_recebimento_bloqueante(f, m);
                        #[cfg(all(feature = "teste-prazo", not(feature = "teste-ciclo-espera")))]
                        let _ = writeln!(
                            crate::io::Emergencia,
                            "[teste-prazo] MENSAGEM id={}",
                            ATUAL + 1
                        );
                    } else if modo == prazo::IMEDIATO {
                        #[cfg(feature = "teste-prazo-empate")]
                        if ATUAL == 0 && TESTE_PRAZO_EMPATE_ARMADO {
                            TESTE_PRAZO_EMPATE_ARMADO = false;
                            crate::io::serial_texto(
                                "[teste-prazo-empate] REGISTRO_RECUSADO motivo=imediato\n",
                            );
                        }
                        f.rax = erro::AUSENTE;
                    } else {
                        match p.espera_ipc.registrar(&mut p.estado, 0) {
                            Ok(true) => {
                                p.recepcao_pendente = true;
                                p.prazo_recepcao = prazo;
                                #[cfg(feature = "teste-prazo-empate")]
                                if ATUAL == 0 && TESTE_PRAZO_EMPATE_ARMADO {
                                    if modo == prazo::FINITO && f.rsi == 40 {
                                        let limite = prazo.unwrap_or(0);
                                        TESTE_PRAZO_EMPATE_ARMADO = false;
                                        TESTE_PRAZO_EMPATE_REGISTRADO = true;
                                        TESTE_PRAZO_EMPATE_TIMEOUT = false;
                                        TESTE_PRAZO_EMPATE_REFERENCIA = p.referencia;
                                        TESTE_PRAZO_EMPATE_PRAZO = limite;
                                        let tick = TIQUES;
                                        let referencia = p.referencia;
                                        let _ = writeln!(
                                            crate::io::Emergencia,
                                            "[teste-prazo-empate] ESPERA referencia={:#x} tick={} prazo={}",
                                            referencia, tick, limite
                                        );
                                    } else {
                                        TESTE_PRAZO_EMPATE_ARMADO = false;
                                        let _ = writeln!(
                                            crate::io::Emergencia,
                                            "[teste-prazo-empate] REGISTRO_RECUSADO motivo=parametros modo={} duracao={}",
                                            modo, f.rsi
                                        );
                                    }
                                }
                                #[cfg(feature = "teste-rz022")]
                                if ATUAL as u64 + 1 == REDE {
                                    TESTE_RZ022_REDE_PENDENTE = false;
                                    TESTE_RZ022_REDE_REFERENCIA = 0;
                                    if rz022_anuncio {
                                        TESTE_RZ022_REDE_REFERENCIA = p.referencia;
                                        TESTE_RZ022_REDE_PENDENTE = true;
                                        if !TESTE_RZ022_REDE_BLOQUEADA {
                                            TESTE_RZ022_REDE_BLOQUEADA = true;
                                            let referencia = p.referencia;
                                            let _ = writeln!(
                                                crate::io::Emergencia,
                                                "[teste-rz022] REDE_BLOQUEADA referencia={:#x} modo=FINITO prazo=10",
                                                referencia
                                            );
                                        }
                                    }
                                }
                                #[cfg(feature = "teste-ciclo-espera")]
                                if let Some(limite) = prazo {
                                    let _ = writeln!(
                                        crate::io::Emergencia,
                                        "[teste-ciclo-espera] BLOQUEADA referencia={:#x} prazo={}",
                                        p.referencia,
                                        limite
                                    );
                                }
                                #[cfg(all(
                                    feature = "teste-prazo",
                                    not(feature = "teste-ciclo-espera")
                                ))]
                                let _ = writeln!(
                                    crate::io::Emergencia,
                                    "[teste-prazo] BLOQUEADO id={}",
                                    ATUAL + 1
                                );
                                return agendar();
                            }
                            Err(e) => {
                                #[cfg(feature = "teste-prazo-empate")]
                                if ATUAL == 0 && TESTE_PRAZO_EMPATE_ARMADO {
                                    TESTE_PRAZO_EMPATE_ARMADO = false;
                                    let _ = writeln!(
                                        crate::io::Emergencia,
                                        "[teste-prazo-empate] REGISTRO_RECUSADO motivo=estado erro={:#x}",
                                        e
                                    );
                                }
                                f.rax = e
                            }
                            Ok(false) => {
                                #[cfg(feature = "teste-prazo-empate")]
                                if ATUAL == 0 && TESTE_PRAZO_EMPATE_ARMADO {
                                    TESTE_PRAZO_EMPATE_ARMADO = false;
                                    crate::io::serial_texto(
                                        "[teste-prazo-empate] REGISTRO_RECUSADO motivo=nao_bloqueou\n",
                                    );
                                }
                                f.rax = erro::ESTADO
                            }
                        }
                    }
                }
                chamada::RECEBER_BLOQUEANTE => {
                    let atuais = referencias_atuais();
                    let p = &mut *addr_of_mut!(PROCESSOS).cast::<Processo>().add(ATUAL);
                    if let Some(m) = retirar_atual(&mut p.fila, &atuais) {
                        preencher_recebimento_bloqueante(f, m);
                        #[cfg(any(
                            feature = "teste-ipc-bloqueante",
                            feature = "teste-receber-bloqueante"
                        ))]
                        let _ = writeln!(
                            crate::io::Emergencia,
                            "[teste-ipc] RECEBER_IMEDIATO id={}",
                            ATUAL + 1
                        );
                    } else {
                        match p.espera_ipc.registrar(&mut p.estado, 0) {
                            Ok(true) => {
                                p.recepcao_pendente = true;
                                #[cfg(any(
                                    feature = "teste-ipc-bloqueante",
                                    feature = "teste-receber-bloqueante"
                                ))]
                                let _ = writeln!(
                                    crate::io::Emergencia,
                                    "[teste-ipc] RECEBER_BLOQUEADO id={}",
                                    ATUAL + 1
                                );
                                return agendar();
                            }
                            Err(e) => f.rax = e,
                            Ok(false) => f.rax = erro::ESTADO,
                        }
                    }
                }
                chamada::ESPERAR_IPC => {
                    f.rax = 0;
                    let p = &mut *addr_of_mut!(PROCESSOS).cast::<Processo>().add(ATUAL);
                    match p.espera_ipc.registrar(&mut p.estado, p.fila.tamanho()) {
                        Ok(true) => {
                            #[cfg(any(
                                feature = "teste-ipc-bloqueante",
                                feature = "teste-receber-bloqueante"
                            ))]
                            let _ = writeln!(
                                crate::io::Emergencia,
                                "[teste-ipc] BLOQUEADO id={}",
                                ATUAL + 1
                            );
                            return agendar();
                        }
                        Ok(false) => {
                            #[cfg(any(
                                feature = "teste-ipc-bloqueante",
                                feature = "teste-receber-bloqueante"
                            ))]
                            let _ = writeln!(
                                crate::io::Emergencia,
                                "[teste-ipc] DISPONIVEL id={}",
                                ATUAL + 1
                            );
                        }
                        Err(e) => f.rax = e,
                    }
                }
                chamada::SAIR => {
                    let id = ATUAL as u64 + 1;
                    cancelar_processo(ATUAL);
                    PROCESSOS[ATUAL].estado.terminar(Vida::Finalizada);
                    publicar_vida(
                        PROCESSOS[ATUAL].referencia,
                        contrato_sistema::estado::FINALIZADO,
                        false,
                    );
                    invalidar_observadores(PROCESSOS[ATUAL].referencia);
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
                chamada::ESTADO_SERVICO => {
                    f.rax = consultar_estado(f.rdi);
                }
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
            cancelar_processo(ATUAL);
            PROCESSOS[ATUAL].estado.terminar(Vida::Falhou);
            publicar_vida(
                PROCESSOS[ATUAL].referencia,
                contrato_sistema::estado::FALHOU,
                false,
            );
            invalidar_observadores(PROCESSOS[ATUAL].referencia);
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
    #[cfg(feature = "teste-prazo-empate")]
    let referencia_destino = PROCESSOS[destino].referencia;
    if !PROCESSOS[destino].estado.ativo() {
        return erro::SERVICO;
    }
    expirar_processo(destino);
    #[cfg(feature = "teste-rx-quota")]
    let atuais_rx_quota = referencias_atuais();
    let p = &mut *addr_of_mut!(PROCESSOS).cast::<Processo>().add(destino);
    let inserir = if id_referencia(f.rdi) == TERMINAL {
        inserir_terminal_e_despertar
    } else {
        inserir_e_despertar
    };
    match inserir(
        &mut p.fila,
        &mut p.espera_ipc,
        &mut p.estado,
        Mensagem {
            origem,
            codigo: f.rsi,
            a: f.rdx,
            b: f.r10,
            c: f.r8,
            d: f.r9,
        },
    ) {
        Ok(despertou) => {
            #[cfg(feature = "teste-prazo-empate")]
            if origem_id == TERMINAL
                && id_referencia(f.rdi) == REDE
                && f.rsi == 0x7f0
                && f.rdx == 0x0505
                && f.r10 == 0
                && f.r8 == 0
                && f.r9 == 0
                && !TESTE_PRAZO_EMPATE_ARMADO
                && !TESTE_PRAZO_EMPATE_REGISTRADO
            {
                TESTE_PRAZO_EMPATE_ARMADO = true;
                let _ = writeln!(
                    crate::io::Emergencia,
                    "[teste-prazo-empate] ARMADA origem={:#x} destino={:#x} token={:#x}",
                    origem,
                    referencia_destino,
                    f.rdx
                );
            }
            #[cfg(feature = "teste-prazo-empate")]
            if origem_id == REDE
                && id_referencia(f.rdi) == TERMINAL
                && f.rsi == 0x7f1
                && f.rdx == TESTE_PRAZO_EMPATE_PRAZO
                && f.r10 == 0x0505
                && f.r8 == 0
                && f.r9 == 0
                && TESTE_PRAZO_EMPATE_REGISTRADO
            {
                let tick = TIQUES;
                let prazo = TESTE_PRAZO_EMPATE_PRAZO;
                let _ = writeln!(
                    crate::io::Emergencia,
                    "[teste-prazo-empate] ADMISSAO origem={:#x} destino={:#x} codigo={:#x} token={:#x} tick={} prazo={}",
                    origem, referencia_destino, f.rsi, f.r10, tick, prazo
                );
                // Correlação one-shot; mantém o prazo congelado para o
                // diagnóstico de resultado até o fim da fixture.
                TESTE_PRAZO_EMPATE_REGISTRADO = false;
            }
            #[cfg(feature = "teste-rz022")]
            if origem_id == REDE
                && id_referencia(f.rdi) == TERMINAL
                && f.rsi == 0x7b1
                && f.r8 == 10
                && f.r9 == 0
            {
                // O serviço anuncia a espera do cenário RZ022 imediatamente
                // antes de RECEBER_COM_PRAZO; isso exclui esperas de reset/poll.
                TESTE_RZ022_REDE_ANUNCIO = true;
            }
            #[cfg(any(feature = "teste-ipc-bloqueante", feature = "teste-receber-bloqueante"))]
            if despertou {
                let _ = writeln!(
                    crate::io::Emergencia,
                    "[teste-ipc] DESPERTADO id={}",
                    destino + 1
                );
            }
            #[cfg(not(feature = "teste-ipc-bloqueante"))]
            let _ = despertou;
            0
        }
        Err(e) => {
            #[cfg(feature = "teste-rx-quota")]
            if e == erro::FILA_CHEIA
                && origem_id == REDE
                && id_referencia(f.rdi) == TERMINAL
                && f.rsi == 0x7ba
                && f.rdx == 16
                && f.r10 == 0x525a32355154
                && !TESTE_RX_QUOTA_FILA_CHEIA_VISTA
                && p.fila
                    .rx_quota_confere(atuais_rx_quota[1], atuais_rx_quota[2], false)
            {
                TESTE_RX_QUOTA_FILA_CHEIA_VISTA = true;
                let _ = writeln!(
                    crate::io::Emergencia,
                    "[teste-rx-quota] FILA_CHEIA origem={:#x} destino={:#x} id=16 codigo={:#x} retorno={:#x} fila=15 quota_rede=15 CPL3",
                    origem,
                    p.referencia,
                    f.rsi,
                    e
                );
            }
            e
        }
    }
}
unsafe fn receber(f: &mut Quadro) {
    let atuais = referencias_atuais();
    let p = &mut *addr_of_mut!(PROCESSOS).cast::<Processo>().add(ATUAL);
    let mensagem = retirar_atual(&mut p.fila, &atuais).unwrap_or_default();
    f.rax = mensagem.codigo;
    f.rdi = mensagem.origem;
    f.rsi = mensagem.a;
    f.rdx = mensagem.b;
    f.r10 = mensagem.c;
    f.r8 = mensagem.d;
    f.r9 = 0;
}

fn preencher_recebimento_bloqueante(f: &mut Quadro, m: Mensagem) {
    f.rax = 0;
    f.rdi = m.origem;
    f.rsi = m.codigo;
    f.rdx = m.a;
    f.r10 = m.b;
    f.r8 = m.c;
    f.r9 = m.d;
}
fn preencher_evento(f: &mut Quadro, evento: EventoEscolhido) {
    match evento {
        EventoEscolhido::Ipc(m) => {
            preencher_recebimento_bloqueante(f, m);
            f.rax = classe_evento::IPC;
        }
        EventoEscolhido::Vida(v) => {
            f.rax = classe_evento::VIDA;
            f.rdi = v.referencia;
            f.rsi = v.tipo;
            f.rdx = v.estado;
            f.r10 = 0;
            f.r8 = 0;
            f.r9 = 0;
        }
    }
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
    if PROCESSOS[indice].estado.vida == Vida::Inexistente {
        return erro::ESTADO;
    }
    // A quota e a preparacao em arena e tabelas inativas precedem a revogacao.
    let (nova, imagem) = match preparar_reinicio(antiga, |nova| {
        let boot = BOOT.ok_or(erro::ESTADO)?;
        #[cfg(feature = "teste-falha-preparacao")]
        let elf_preparar = if geracao_referencia(nova) == 2 {
            // Falha injetada no próprio parser ELF usado pela preparação real;
            // a arena/tabelas da geração não são publicadas neste retorno.
            &[][..]
        } else {
            REDE_ELF
        };
        #[cfg(not(feature = "teste-falha-preparacao"))]
        let elf_preparar = REDE_ELF;
        memoria::carregar(indice, elf_preparar, &boot, plataforma::mmio_atribuido(), nova)
            .map_err(|e| {
                #[cfg(feature = "teste-falha-preparacao")]
                if geracao_referencia(nova) == 2 && e == "ELF de servico invalido" {
                    let _ = writeln!(
                        crate::io::Emergencia,
                        "[teste-falha-preparacao] ERRO_REAL_LOADER antiga={:#x} nova={:#x} motivo=ELF_INVALIDO",
                        antiga,
                        nova
                    );
                }
                let _ = writeln!(
                    crate::io::Emergencia,
                    "[microkernel] REINICIO_RECUSADO referencia={:#x} motivo={}",
                    antiga,
                    e
                );
                erro::IMAGEM
            })
    }) {
        Ok(preparado) => preparado,
        Err(e) => return e,
    };
    let arena_antiga = PROCESSOS[indice].fisico;
    plataforma::desativar_rede();
    cancelar_processo(indice);
    PROCESSOS[indice].estado.terminar(Vida::Finalizada);
    // Invalida a identidade e conserva ultimo estado observado antes da troca;
    // a finalizacao interna da substituicao nao e nova transicao publicada.
    publicar_vida(antiga, 0, true);
    invalidar_observadores(antiga);
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
    if !PROCESSOS[indice].estado.ativo() {
        return erro::ESTADO;
    }
    plataforma::desativar_rede();
    cancelar_processo(indice);
    PROCESSOS[indice].estado.terminar(Vida::Finalizada);
    publicar_vida(referencia, contrato_sistema::estado::FINALIZADO, false);
    invalidar_observadores(referencia);
    let purgadas = purgar_instancia(indice, referencia);
    let _ = writeln!(
        crate::io::Emergencia,
        "[microkernel] SERVICO_ENCERRADO id=2 referencia={:#x} purgadas={}",
        referencia,
        purgadas
    );
    0
}
