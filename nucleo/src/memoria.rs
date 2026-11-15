//! Espacos de endereco independentes. Identidade supervisor e janelas CPL3 minimas.
use contrato_boot::{DescritorMemoria, InformacoesBoot, Tela};
use contrato_sistema::*;
use core::{
    mem,
    ptr::{addr_of_mut, copy_nonoverlapping, write, write_bytes},
};
const PAGINA: u64 = 4096;
const NX: u64 = 1 << 63;
const LIMITE_IMAGEM: u64 = INICIO_PILHA - PAGINA; // pagina de guarda inferior.
const QUANTIDADE_ARENAS: usize = 2 + MAX_INSTANCIAS_REDE as usize;

#[repr(C, align(4096))]
#[derive(Clone, Copy)]
struct Pagina([u64; 512]);
#[repr(C, align(4096))]
#[derive(Clone, Copy)]
struct Tabelas {
    pml4: Pagina,
    pdpt: Pagina,
    diretorios: [Pagina; 4],
    usuario: Pagina,
    mmio: Pagina,
    video: [Pagina; 8],
}
const ZERO: Pagina = Pagina([0; 512]);
static mut ESPACOS: [Tabelas; QUANTIDADE_ARENAS] = [Tabelas {
    pml4: ZERO,
    pdpt: ZERO,
    diretorios: [ZERO; 4],
    usuario: ZERO,
    mmio: ZERO,
    video: [ZERO; 8],
}; QUANTIDADE_ARENAS];
#[repr(C, align(4096))]
#[derive(Clone, Copy)]
struct Arena([u8; TAMANHO_USUARIO]);
static mut ARENAS: [Arena; QUANTIDADE_ARENAS] = [Arena([0; TAMANHO_USUARIO]); QUANTIDADE_ARENAS];

pub struct Imagem {
    pub entrada: u64,
    pub cr3: u64,
    pub fisico: u64,
    pub tamanho: u64,
}
fn u16le(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes(b[i..i + 2].try_into().unwrap())
}
fn u32le(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes(b[i..i + 4].try_into().unwrap())
}
fn u64le(b: &[u8], i: usize) -> u64 {
    u64::from_le_bytes(b[i..i + 8].try_into().unwrap())
}

/// Prepara uma instancia ainda nao publicada. Cada encarnacao da rede possui
/// arena e tabelas exclusivas; uma arena publicada jamais e reutilizada no boot.
/// O chamador executa com IF=0 e garante que esta referencia ainda nao foi publicada.
pub unsafe fn carregar(
    indice: usize,
    b: &[u8],
    boot: &InformacoesBoot,
    mmio: Option<u64>,
    referencia: u64,
) -> Result<Imagem, &'static str> {
    let arena = nucleo::controle::arena_da_instancia(referencia)
        .map_err(|_| "referencia de arena invalida")?;
    if id_referencia(referencia) != indice as u64 + 1 {
        return Err("referencia nao pertence ao servico");
    }
    if indice >= QUANTIDADE_SERVICOS
        || b.len() < 64
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
        return Err("ELF de servico invalido");
    }
    let quantidade = u16le(b, 56) as usize;
    let tabela = usize::try_from(u64le(b, 32)).map_err(|_| "tabela ELF")?;
    if quantidade == 0
        || quantidade > 32
        || tabela
            .checked_add(quantidade * 56)
            .is_none_or(|e| e > b.len())
    {
        return Err("tabela ELF fora dos limites");
    }
    let entrada = u64le(b, 24);
    let mut entrada_valida = false;
    let mut fim_imagem = BASE_USUARIO;
    // Valida tudo antes de copiar; o ELF nao recebe acesso ao endereco fisico solicitado.
    for n in 0..quantidade {
        let h = tabela + n * 56;
        let tipo = u32le(b, h);
        if tipo == 2 || tipo == 3 {
            return Err("servico dinamico nao suportado");
        }
        if tipo != 1 {
            continue;
        }
        let flags = u32le(b, h + 4);
        let offset = u64le(b, h + 8);
        let v = u64le(b, h + 16);
        let arquivo = u64le(b, h + 32);
        let memoria = u64le(b, h + 40);
        let align = u64le(b, h + 48);
        if arquivo > memoria
            || v < BASE_USUARIO
            || v.checked_add(memoria).is_none_or(|e| e > LIMITE_IMAGEM)
            || offset
                .checked_add(arquivo)
                .is_none_or(|e| e > b.len() as u64)
            || flags & 3 == 3
            || (align > 1 && (!align.is_power_of_two() || v % align != offset % align))
        {
            return Err("segmento ELF fora da reserva ou W+X");
        }
        if memoria == 0 {
            continue;
        }
        for antes in 0..n {
            let a = tabela + antes * 56;
            if u32le(b, a) == 1 {
                let av = u64le(b, a + 16);
                let am = u64le(b, a + 40);
                if am > 0 && v < av + am && av < v + memoria {
                    return Err("segmentos ELF sobrepostos");
                }
            }
        }
        if flags & 1 != 0 && entrada >= v && entrada < v + arquivo {
            entrada_valida = true;
        }
        fim_imagem = fim_imagem.max(v + memoria);
    }
    if !entrada_valida {
        return Err("entrada de servico sem pagina executavel");
    }
    let fisico = addr_of_mut!(ARENAS).cast::<Arena>().add(arena) as u64;
    let tabelas = addr_of_mut!(ESPACOS).cast::<Tabelas>().add(arena);
    // Uma tentativa anterior que nao foi publicada pode ter falhado na validacao
    // de permissoes. Nao permitir PTEs ou dados residuais em uma nova preparacao.
    write_bytes(tabelas, 0, 1);
    let t = &mut *tabelas;
    write_bytes(fisico as *mut u8, 0, TAMANHO_USUARIO);
    // PML4 permite descida para CPL3; cada ramo supervisor bloqueia o acesso.
    t.pml4.0[0] = addr_of_mut!(t.pdpt) as u64 | 7;
    for gi in 0..4 {
        t.pdpt.0[gi] = addr_of_mut!(t.diretorios[gi]) as u64 | if gi == 1 { 7 } else { 3 };
        for n in 0..512 {
            let p = ((gi * 512 + n) as u64) * 0x20_0000;
            let mmio_aqui = mmio.is_some_and(|base| p < base + 128 * 1024 && base < p + 0x20_0000);
            let video_aqui = boot.tela.endereco != 0
                && p < boot.tela.endereco.saturating_add(boot.tela.tamanho)
                && boot.tela.endereco < p + 0x20_0000;
            let cache = if p >= 0xc000_0000 || mmio_aqui || video_aqui {
                0x18
            } else {
                0
            };
            t.diretorios[gi].0[n] = p | 0x83 | cache; // supervisor em todas as folhas identidade.
        }
    }
    t.diretorios[1].0[0] = addr_of_mut!(t.usuario) as u64 | 7;
    for n in 0..quantidade {
        let h = tabela + n * 56;
        if u32le(b, h) != 1 {
            continue;
        }
        let flags = u32le(b, h + 4);
        let offset = u64le(b, h + 8);
        let v = u64le(b, h + 16);
        let arquivo = u64le(b, h + 32);
        let memoria = u64le(b, h + 40);
        if memoria == 0 {
            continue;
        }
        let permissoes =
            5 | if flags & 2 != 0 { 2 } else { 0 } | if flags & 1 == 0 { NX } else { 0 };
        let inicio = (v - BASE_USUARIO) / PAGINA;
        let fim = (v - BASE_USUARIO + memoria).div_ceil(PAGINA);
        for pagina in inicio..fim {
            let pte = (fisico + pagina * PAGINA) | permissoes;
            let anterior = t.usuario.0[pagina as usize];
            if anterior != 0 && anterior != pte {
                return Err("permissoes ELF conflitam na mesma pagina");
            }
            t.usuario.0[pagina as usize] = pte;
        }
        copy_nonoverlapping(
            b.as_ptr().add(offset as usize),
            (fisico + v - BASE_USUARIO) as *mut u8,
            arquivo as usize,
        );
    }
    for v in (INICIO_PILHA..TOPO_PILHA).step_by(PAGINA as usize) {
        t.usuario.0[((v - BASE_USUARIO) / PAGINA) as usize] = (fisico + v - BASE_USUARIO) | 7 | NX;
    }
    // Guardas INICIO_PILHA-4096 e TOPO_PILHA permanecem sem PTE.
    t.usuario.0[((INFO_USUARIO - BASE_USUARIO) / PAGINA) as usize] =
        (fisico + INFO_USUARIO - BASE_USUARIO) | 5 | NX;
    let mut tela = Tela::default();
    if indice == 0
        && boot.tela.endereco != 0
        && boot.tela.endereco % PAGINA == 0
        && boot.tela.tamanho > 0
        && boot.tela.tamanho <= 16 * 1024 * 1024
    {
        tela = boot.tela;
        tela.endereco = FRAMEBUFFER_USUARIO;
        for pagina in 0..boot.tela.tamanho.div_ceil(PAGINA) as usize {
            let d = pagina / 512;
            let p = pagina % 512;
            t.diretorios[1].0[256 + d] = addr_of_mut!(t.video[d]) as u64 | 7;
            t.video[d].0[p] = (boot.tela.endereco + pagina as u64 * PAGINA) | 0x1f | NX;
        }
    }
    if indice == 1 {
        if let Some(base) = mmio {
            if base % PAGINA != 0
                || base
                    .checked_add(128 * 1024)
                    .is_none_or(|e| e > 0x1_0000_0000)
            {
                return Err("MMIO fora da plataforma");
            }
            t.diretorios[1].0[128] = addr_of_mut!(t.mmio) as u64 | 7;
            for pagina in 0..32 {
                t.mmio.0[pagina] = (base + pagina as u64 * PAGINA) | 0x1f | NX;
            }
        }
    }
    let info = InfoServico {
        assinatura: ASSINATURA_SERVICO,
        versao: VERSAO_ABI,
        id: indice as u64 + 1,
        referencia,
        base_fisica: fisico,
        tamanho_imagem: fim_imagem - BASE_USUARIO,
        tela,
        memoria_livre_boot: memoria_livre_boot(boot),
        nucleo_inicio: boot.nucleo_inicio,
        nucleo_fim: boot.nucleo_fim,
    };
    assert!(mem::size_of::<InfoServico>() <= PAGINA as usize);
    write(
        (fisico + INFO_USUARIO - BASE_USUARIO) as *mut InfoServico,
        info,
    );
    // Imita uma chamada SysV sem retorno para a entrada de userspace.
    write((fisico + TOPO_PILHA - BASE_USUARIO - 8) as *mut u64, 0);
    Ok(Imagem {
        entrada,
        cr3: addr_of_mut!(t.pml4) as u64,
        fisico,
        tamanho: fim_imagem - BASE_USUARIO,
    })
}

fn memoria_livre_boot(boot: &InformacoesBoot) -> u64 {
    let mut total = 0u64;
    for i in 0..boot.mapa_tamanho / boot.descritor_tamanho {
        let d = unsafe {
            core::ptr::read_unaligned(
                (boot.mapa_endereco + i * boot.descritor_tamanho) as *const DescritorMemoria,
            )
        };
        if d.tipo == 7 {
            total = total.saturating_add(d.paginas.saturating_mul(4096));
        }
    }
    total
}
