#![no_std]
#![no_main]

mod elf;
mod uefi;

use contrato_boot::{InformacoesBoot, Tela, ASSINATURA, VERSAO};
use core::{arch::asm, ffi::c_void, mem, ptr};
use uefi::*;

static NUCLEO: &[u8] = include_bytes!(env!("RAIZ_KERNEL"));
const PAGINA: usize = 4096;
const MAPA_CAPACIDADE: usize = 256 * 1024;
const PILHA_PAGINAS: usize = 32;

#[no_mangle]
pub unsafe extern "efiapi" fn efi_main(imagem: *mut c_void, sistema: *mut TabelaSistema) -> usize {
    if sistema.is_null() || (*sistema).servicos_boot.is_null() {
        return ERRO_PARAMETRO;
    }
    let bs = &*(*sistema).servicos_boot;
    mensagem(
        sistema,
        concat!(
            "Raiz OS ",
            env!("CARGO_PKG_VERSION"),
            " - carregador proprio em Rust\r\n"
        ),
    );
    // O watchdog do firmware nao deve encerrar a sessao do nosso sistema.
    let watchdog: Watchdog = mem::transmute(bs.funcoes[29]);
    let _ = watchdog(0, 0, 0, ptr::null());

    let plano = match elf::planejar(NUCLEO) {
        Ok(p) => p,
        Err(_) => {
            mensagem(sistema, "ERRO: ELF do nucleo invalido.\r\n");
            return ERRO_CARGA;
        }
    };
    let alocar: AlocarPaginas = mem::transmute(bs.funcoes[2]);
    let liberar: LiberarPaginas = mem::transmute(bs.funcoes[3]);
    let paginas_nucleo = ((plano.fim - plano.inicio) as usize).div_ceil(PAGINA);
    let mut endereco = plano.inicio;
    if alocar(2, 1, paginas_nucleo, &mut endereco) != 0 {
        mensagem(sistema, "ERRO: memoria fixa do nucleo indisponivel.\r\n");
        return ERRO_MEMORIA;
    }
    elf::copiar(NUCLEO, &plano);

    let pilha = match alocar_abaixo_4g(alocar, PILHA_PAGINAS) {
        Some(e) => e,
        None => {
            liberar(plano.inicio, paginas_nucleo);
            return ERRO_MEMORIA;
        }
    };
    let info_endereco = match alocar_abaixo_4g(alocar, 1) {
        Some(e) => e,
        None => {
            liberar(pilha, PILHA_PAGINAS);
            liberar(plano.inicio, paginas_nucleo);
            return ERRO_MEMORIA;
        }
    };
    let mapa_endereco = match alocar_abaixo_4g(alocar, MAPA_CAPACIDADE / PAGINA) {
        Some(e) => e,
        None => {
            liberar(info_endereco, 1);
            liberar(pilha, PILHA_PAGINAS);
            liberar(plano.inicio, paginas_nucleo);
            return ERRO_MEMORIA;
        }
    };
    let informacoes = info_endereco as *mut InformacoesBoot;
    ptr::write(
        informacoes,
        InformacoesBoot {
            assinatura: ASSINATURA,
            versao: VERSAO,
            tamanho: mem::size_of::<InformacoesBoot>() as u32,
            tela: obter_tela(bs),
            mapa_endereco,
            mapa_tamanho: 0,
            descritor_tamanho: 0,
            descritor_versao: 0,
            reservado: 0,
            nucleo_inicio: plano.inicio,
            nucleo_fim: plano.fim,
            pilha_inicio: pilha,
            pilha_fim: pilha + (PILHA_PAGINAS * PAGINA) as u64,
        },
    );

    mensagem(sistema, "ELF validado; segmentos e pilha preparados.\r\n");
    mensagem(sistema, "Encerrando servicos de boot do UEFI...\r\n");
    let mapa: ObterMapa = mem::transmute(bs.funcoes[4]);
    let sair: SairBoot = mem::transmute(bs.funcoes[26]);
    // Depois da primeira tentativa de saida, somente mapa/ExitBootServices.
    // Nenhum log de firmware, alocacao ou protocolo entre mapa e saida.
    for _ in 0..8 {
        let mut tamanho = MAPA_CAPACIDADE;
        let mut chave = 0usize;
        let mut stride = 0usize;
        let mut versao = 0u32;
        let status = mapa(
            &mut tamanho,
            mapa_endereco as *mut c_void,
            &mut chave,
            &mut stride,
            &mut versao,
        );
        if status != 0
            || stride < mem::size_of::<contrato_boot::DescritorMemoria>()
            || tamanho % stride != 0
        {
            parar();
        }
        (*informacoes).mapa_tamanho = tamanho as u64;
        (*informacoes).descritor_tamanho = stride as u64;
        (*informacoes).descritor_versao = versao;
        let status = sair(imagem, chave);
        if status == 0 {
            transferir(plano.entrada, informacoes, (*informacoes).pilha_fim);
        }
        if status != ERRO_PARAMETRO {
            parar();
        }
    }
    parar()
}

unsafe fn alocar_abaixo_4g(alocar: AlocarPaginas, paginas: usize) -> Option<u64> {
    let mut endereco = 0xffff_ffffu64;
    if alocar(1, 2, paginas, &mut endereco) == 0 {
        Some(endereco)
    } else {
        None
    }
}

unsafe fn obter_tela(bs: &ServicosBoot) -> Tela {
    let localizar: LocalizarProtocolo = mem::transmute(bs.funcoes[37]);
    let mut protocolo: *mut c_void = ptr::null_mut();
    if localizar(&GUID_GOP, ptr::null_mut(), &mut protocolo) != 0 || protocolo.is_null() {
        return Tela::default();
    }
    let modo = (*(protocolo as *const SaidaGrafica)).modo;
    if modo.is_null() || (*modo).informacoes.is_null() {
        return Tela::default();
    }
    let i = &*(*modo).informacoes;
    let t = Tela {
        endereco: (*modo).framebuffer,
        tamanho: (*modo).framebuffer_tamanho as u64,
        largura: i.largura,
        altura: i.altura,
        pixels_por_linha: i.pixels_por_linha,
        formato: i.formato,
    };
    let necessario = (t.pixels_por_linha as u64)
        .checked_mul(t.altura as u64)
        .and_then(|n| n.checked_mul(4));
    if t.formato > 1
        || t.largura == 0
        || t.altura == 0
        || t.pixels_por_linha < t.largura
        || necessario.is_none()
        || necessario.unwrap() > t.tamanho
        || t.endereco
            .checked_add(t.tamanho)
            .is_none_or(|fim| fim > 0x1_0000_0000)
    {
        Tela::default()
    } else {
        t
    }
}

unsafe fn mensagem(sistema: *mut TabelaSistema, texto: &str) {
    let saida = (*sistema).saida;
    if saida.is_null() {
        return;
    }
    let mut buffer = [0u16; 160];
    let mut n = 0;
    for c in texto.encode_utf16() {
        if n + 1 >= buffer.len() {
            break;
        }
        buffer[n] = c;
        n += 1;
    }
    ((*saida).escrever)(saida, buffer.as_ptr());
}

unsafe fn transferir(entrada: u64, info: *const InformacoesBoot, pilha: u64) -> ! {
    // A entrada ELF segue System V: rdi = informacoes e rsp % 16 == 8.
    // Esta ponte encerra a ABI efiapi (Win64) e nunca retorna ao firmware.
    asm!("cli", "cld", "mov rsp, {pilha}", "and rsp, -16", "xor rbp, rbp",
        "sub rsp, 8", "mov qword ptr [rsp], 0", "jmp {entrada}",
        pilha = in(reg) pilha, entrada = in(reg) entrada, in("rdi") info, options(noreturn));
}

fn parar() -> ! {
    loop {
        unsafe {
            asm!("cli", "hlt", options(nomem, nostack));
        }
    }
}

#[panic_handler]
fn panico(_: &core::panic::PanicInfo) -> ! {
    parar()
}
