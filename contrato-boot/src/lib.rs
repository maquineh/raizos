#![no_std]

//! Contrato próprio, versionado, entre o carregador e o núcleo.
//! Endereços são físicos e mapeados por identidade no protótipo.

pub const ASSINATURA: u64 = 0x5241_495a_424f_4f54;
pub const VERSAO: u32 = 1;
pub const BASE_NUCLEO: u64 = 0x0200_0000;
pub const LIMITE_NUCLEO: u64 = 0x0400_0000;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Tela {
    pub endereco: u64,
    pub tamanho: u64,
    pub largura: u32,
    pub altura: u32,
    pub pixels_por_linha: u32,
    /// UEFI GOP: 0 = RGB reservado; 1 = BGR reservado. Outros rejeitados.
    pub formato: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct InformacoesBoot {
    pub assinatura: u64,
    pub versao: u32,
    pub tamanho: u32,
    pub tela: Tela,
    pub mapa_endereco: u64,
    pub mapa_tamanho: u64,
    pub descritor_tamanho: u64,
    pub descritor_versao: u32,
    pub reservado: u32,
    pub nucleo_inicio: u64,
    pub nucleo_fim: u64,
    pub pilha_inicio: u64,
    pub pilha_fim: u64,
}

/// Prefixo padronizado do descritor UEFI. Sempre avançar pelo stride recebido.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DescritorMemoria {
    pub tipo: u32,
    pub reservado: u32,
    pub inicio_fisico: u64,
    pub inicio_virtual: u64,
    pub paginas: u64,
    pub atributos: u64,
}
