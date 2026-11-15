#![no_std]
#![no_main]

mod cpu;
mod io;
mod memoria;
mod plataforma;
mod processos;

use contrato_boot::{InformacoesBoot, ASSINATURA, VERSAO};
use core::fmt::Write;

#[no_mangle]
#[link_section = ".text.entrada"]
pub unsafe extern "sysv64" fn _inicio_nucleo(informacoes: *const InformacoesBoot) -> ! {
    io::iniciar_serial();
    io::serial_texto(
        "\n[boot] EXIT_BOOT_SERVICES_OK\n[microkernel] Raiz OS: nucleo proprio Rust.\n",
    );
    if informacoes.is_null() {
        io::serial_texto("Contrato de boot ausente.\n");
        io::parar();
    }
    let boot = *informacoes;
    if boot.assinatura != ASSINATURA
        || boot.versao != VERSAO
        || boot.tamanho as usize != core::mem::size_of::<InformacoesBoot>()
        || boot.descritor_tamanho < core::mem::size_of::<contrato_boot::DescritorMemoria>() as u64
        || boot.mapa_tamanho > 256 * 1024
        || boot.mapa_tamanho % boot.descritor_tamanho != 0
        || boot
            .mapa_endereco
            .checked_add(boot.mapa_tamanho)
            .is_none_or(|e| e > 0x1_0000_0000)
    {
        io::serial_texto("Contrato de boot invalido.\n");
        io::parar();
    }
    cpu::iniciar();
    let mmio = plataforma::descobrir();
    if let Some(bdf) = plataforma::bdf_atribuido() {
        let _ = writeln!(
            io::Emergencia,
            "[microkernel] PCI atribuido ao servico 2: BDF={:#x} MMIO={:#x}",
            bdf,
            plataforma::mmio_atribuido().unwrap_or(0)
        );
    } else {
        io::serial_texto("[microkernel] Nenhuma e1000 compativel; terminal continua disponivel.\n");
    }
    if let Err(e) = processos::iniciar(&boot, mmio) {
        let _ = writeln!(
            io::Emergencia,
            "[microkernel] Erro carregando servicos: {}",
            e
        );
        io::parar();
    }
    io::serial_texto(
        "[microkernel] PRONTO: 3 processos CPL3; CR3 distintos; W^X/NX; IPC; PIT 100Hz; ABI 2.\n",
    );
    processos::executar()
}

#[panic_handler]
fn panico(info: &core::panic::PanicInfo) -> ! {
    let _ = writeln!(io::Emergencia, "\n[microkernel] PANICO {}", info);
    io::parar()
}
