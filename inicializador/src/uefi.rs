//! Subconjunto próprio das estruturas da especificação UEFI.
use core::ffi::c_void;

pub const ERRO_PARAMETRO: usize = (1usize << 63) | 2;
pub const ERRO_CARGA: usize = (1usize << 63) | 1;
pub const ERRO_MEMORIA: usize = (1usize << 63) | 9;

#[repr(C)]
pub struct Cabecalho {
    assinatura: u64,
    revisao: u32,
    tamanho: u32,
    crc: u32,
    reservado: u32,
}

#[repr(C)]
pub struct TabelaSistema {
    cabecalho: Cabecalho,
    fabricante: *const u16,
    revisao: u32,
    entrada_handle: *mut c_void,
    entrada: *mut c_void,
    saida_handle: *mut c_void,
    pub saida: *mut SaidaTexto,
    erro_handle: *mut c_void,
    erro: *mut c_void,
    runtime: *mut c_void,
    pub servicos_boot: *const ServicosBoot,
    quantidade_tabelas: usize,
    tabelas: *mut c_void,
}

#[repr(C)]
pub struct SaidaTexto {
    reset: usize,
    pub escrever: unsafe extern "efiapi" fn(*mut SaidaTexto, *const u16) -> usize,
}

#[repr(C)]
pub struct ServicosBoot {
    cabecalho: Cabecalho,
    /// Ordem de EFI_BOOT_SERVICES, UEFI 2.10 secao 4.4.
    pub funcoes: [usize; 44],
}

pub type AlocarPaginas = unsafe extern "efiapi" fn(u32, u32, usize, *mut u64) -> usize;
pub type LiberarPaginas = unsafe extern "efiapi" fn(u64, usize) -> usize;
pub type ObterMapa =
    unsafe extern "efiapi" fn(*mut usize, *mut c_void, *mut usize, *mut usize, *mut u32) -> usize;
pub type SairBoot = unsafe extern "efiapi" fn(*mut c_void, usize) -> usize;
pub type Watchdog = unsafe extern "efiapi" fn(usize, u64, usize, *const u16) -> usize;
pub type LocalizarProtocolo =
    unsafe extern "efiapi" fn(*const Guid, *mut c_void, *mut *mut c_void) -> usize;

#[repr(C, align(8))]
pub struct Guid {
    a: u32,
    b: u16,
    c: u16,
    d: [u8; 8],
}
pub const GUID_GOP: Guid = Guid {
    a: 0x9042_a9de,
    b: 0x23dc,
    c: 0x4a38,
    d: [0x96, 0xfb, 0x7a, 0xde, 0xd0, 0x80, 0x51, 0x6a],
};

#[repr(C)]
pub struct SaidaGrafica {
    consulta: usize,
    alterar: usize,
    blt: usize,
    pub modo: *const ModoGrafico,
}
#[repr(C)]
pub struct ModoGrafico {
    maximo: u32,
    atual: u32,
    pub informacoes: *const InformacaoGrafica,
    informacao_tamanho: usize,
    pub framebuffer: u64,
    pub framebuffer_tamanho: usize,
}
#[repr(C)]
pub struct InformacaoGrafica {
    versao: u32,
    pub largura: u32,
    pub altura: u32,
    pub formato: u32,
    mascaras: [u32; 4],
    pub pixels_por_linha: u32,
}
