//! Atribuicao de hardware: configura PCI e delega uma janela MMIO ao servico 2.
//! O microkernel nao implementa Ethernet, ARP, IP, ICMP, UDP ou o driver e1000.
use contrato_sistema::{erro, MMIO_REDE, REDE};
use core::arch::asm;
#[derive(Clone, Copy)]
struct Dispositivo {
    bdf: u32,
    mmio: u64,
}
static mut ATRIBUIDO: Option<Dispositivo> = None;
unsafe fn saida32(porta: u16, v: u32) {
    asm!("out dx, eax",in("dx")porta,in("eax")v,options(nomem,nostack,preserves_flags));
}
unsafe fn entrada32(porta: u16) -> u32 {
    let v;
    asm!("in eax, dx",in("dx")porta,out("eax")v,options(nomem,nostack,preserves_flags));
    v
}
fn endereco(bdf: u32, reg: u32) -> u32 {
    let bus = (bdf >> 16) & 255;
    let dev = (bdf >> 8) & 31;
    let func = bdf & 7;
    0x8000_0000 | (bus << 16) | (dev << 11) | (func << 8) | (reg & 0xfc)
}
unsafe fn ler_bruto(bdf: u32, reg: u32) -> u32 {
    saida32(0xcf8, endereco(bdf, reg));
    entrada32(0xcfc)
}
unsafe fn comando_bruto(bdf: u32, comando: u16) {
    // Escrever zeros em Status evita limpar bits W1C do dispositivo.
    saida32(0xcf8, endereco(bdf, 4));
    saida32(0xcfc, comando as u32);
}
pub unsafe fn descobrir() -> Option<u64> {
    for bus in 0..256u32 {
        for dev in 0..32u32 {
            let inicial = (bus << 16) | (dev << 8);
            if ler_bruto(inicial, 0) & 0xffff == 0xffff {
                continue;
            }
            let funcoes = if ler_bruto(inicial, 0x0c) & 0x0080_0000 != 0 {
                8
            } else {
                1
            };
            for func in 0..funcoes {
                let bdf = inicial | func;
                if ler_bruto(bdf, 0) != 0x100e_8086 {
                    continue;
                }
                let bar = ler_bruto(bdf, 0x10);
                if bar & 1 != 0 {
                    continue;
                }
                let tipo = (bar >> 1) & 3;
                if tipo != 0 && tipo != 2 {
                    continue;
                }
                let alto = if tipo == 2 {
                    ler_bruto(bdf, 0x14) as u64
                } else {
                    0
                };
                let mmio = (alto << 32) | ((bar & !15) as u64);
                if mmio == 0
                    || mmio % 4096 != 0
                    || mmio
                        .checked_add(128 * 1024)
                        .is_none_or(|e| e > 0x1_0000_0000)
                {
                    continue;
                }
                // A descoberta so habilita MMIO. O driver em CPL3 habilita DMA
                // depois de resetar o dispositivo e preparar os novos aneis.
                let comando = (ler_bruto(bdf, 4) as u16 & !0x0407) | 0x0402;
                comando_bruto(bdf, comando);
                ATRIBUIDO = Some(Dispositivo { bdf, mmio });
                return Some(mmio);
            }
        }
    }
    None
}

pub unsafe fn ler(id: u64, bdf: u64, reg: u64) -> u64 {
    let Some(d) = ATRIBUIDO else {
        return erro::PERMISSAO;
    };
    if id != REDE || bdf != d.bdf as u64 || !matches!(reg, 0 | 4 | 8 | 12 | 16 | 20 | 60) {
        return erro::PERMISSAO;
    }
    match reg {
        16 => MMIO_REDE | (ler_bruto(d.bdf, 16) as u64 & 15),
        20 => 0, // BAR alto virtual: a janela delegada esta abaixo de 4 GiB.
        _ => ler_bruto(d.bdf, reg as u32) as u64,
    }
}
pub unsafe fn escrever_comando(id: u64, bdf: u64, valor: u64) -> u64 {
    let Some(d) = ATRIBUIDO else {
        return erro::PERMISSAO;
    };
    // A rede pode apenas ligar/desligar bus mastering. MMIO continua habilitado
    // e INTx continua desabilitado: este marco nao delega interrupcoes do NIC.
    if id != REDE || bdf != d.bdf as u64 || !matches!(valor, 0x0402 | 0x0406) {
        return erro::PERMISSAO;
    }
    let atual = ler_bruto(d.bdf, 4) as u16;
    comando_bruto(d.bdf, (atual & !0x0406) | (valor as u16));
    0
}
pub unsafe fn desativar_rede() {
    if let Some(d) = ATRIBUIDO {
        let c = ler_bruto(d.bdf, 4) as u16;
        comando_bruto(d.bdf, (c & !4) | 0x0400);
        // Read-back ordena a escrita de configuracao; nao prova drenagem de DMA.
        let _ = ler_bruto(d.bdf, 4);
    }
}
pub fn bdf_atribuido() -> Option<u32> {
    unsafe { ATRIBUIDO.map(|d| d.bdf) }
}
pub fn mmio_atribuido() -> Option<u64> {
    unsafe { ATRIBUIDO.map(|d| d.mmio) }
}
