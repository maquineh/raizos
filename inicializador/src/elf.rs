//! Carregador ELF64 ET_EXEC: valida todos os segmentos antes de copiar qualquer byte.
use contrato_boot::{BASE_NUCLEO, LIMITE_NUCLEO};

pub struct Plano {
    pub inicio: u64,
    pub fim: u64,
    pub entrada: u64,
    tabela: usize,
    quantidade: usize,
}
fn u16le(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
fn u32le(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes(b[i..i + 4].try_into().unwrap())
}
fn u64le(b: &[u8], i: usize) -> u64 {
    u64::from_le_bytes(b[i..i + 8].try_into().unwrap())
}

pub fn planejar(b: &[u8]) -> Result<Plano, &'static str> {
    if b.len() < 64
        || &b[..4] != b"\x7fELF"
        || b[4] != 2
        || b[5] != 1
        || b[6] != 1
        || u16le(b, 16) != 2
        || u16le(b, 18) != 62
        || u32le(b, 20) != 1
        || u16le(b, 52) != 64
        || u16le(b, 54) != 56
    {
        return Err("cabecalho ELF64 invalido");
    }
    let tabela = usize::try_from(u64le(b, 32)).map_err(|_| "offset")?;
    let quantidade = u16le(b, 56) as usize;
    if quantidade == 0
        || quantidade > 32
        || tabela
            .checked_add(quantidade * 56)
            .is_none_or(|fim| fim > b.len())
    {
        return Err("tabela de segmentos invalida");
    }
    let entrada = u64le(b, 24);
    let mut inicio = u64::MAX;
    let mut fim = 0;
    let mut entrada_valida = false;
    for n in 0..quantidade {
        let h = tabela + n * 56;
        let tipo = u32le(b, h);
        if tipo == 2 || tipo == 3 {
            return Err("ligacao dinamica nao suportada");
        }
        if tipo != 1 {
            continue;
        }
        let offset = u64le(b, h + 8);
        let v = u64le(b, h + 16);
        let p = u64le(b, h + 24);
        let arquivo = u64le(b, h + 32);
        let memoria = u64le(b, h + 40);
        let alinhamento = u64le(b, h + 48);
        if arquivo > memoria
            || v != p
            || p < BASE_NUCLEO
            || p.checked_add(memoria).is_none_or(|e| e > LIMITE_NUCLEO)
            || offset
                .checked_add(arquivo)
                .is_none_or(|e| e > b.len() as u64)
            || (alinhamento > 1
                && (!alinhamento.is_power_of_two() || (v % alinhamento) != (offset % alinhamento)))
        {
            return Err("segmento invalido");
        }
        if memoria == 0 {
            continue;
        }
        for anterior in 0..n {
            let a = tabela + anterior * 56;
            if u32le(b, a) == 1 {
                let ap = u64le(b, a + 24);
                let am = u64le(b, a + 40);
                if am > 0 && p < ap + am && ap < p + memoria {
                    return Err("segmentos sobrepostos");
                }
            }
        }
        inicio = inicio.min(p & !4095);
        fim = fim.max((p + memoria + 4095) & !4095);
        if u32le(b, h + 4) & 1 != 0 && entrada >= p && entrada < p + arquivo {
            entrada_valida = true;
        }
    }
    if !entrada_valida || inicio != BASE_NUCLEO || fim <= inicio {
        return Err("entrada invalida");
    }
    Ok(Plano {
        inicio,
        fim,
        entrada,
        tabela,
        quantidade,
    })
}

/// # Safety
/// O chamador reservou exclusivamente toda a faixa do Plano via UEFI.
pub unsafe fn copiar(b: &[u8], p: &Plano) {
    core::ptr::write_bytes(p.inicio as *mut u8, 0, (p.fim - p.inicio) as usize);
    for n in 0..p.quantidade {
        let h = p.tabela + n * 56;
        if u32le(b, h) != 1 {
            continue;
        }
        core::ptr::copy_nonoverlapping(
            b.as_ptr().add(u64le(b, h + 8) as usize),
            u64le(b, h + 24) as *mut u8,
            u64le(b, h + 32) as usize,
        );
    }
}
