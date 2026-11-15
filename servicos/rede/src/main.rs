#![no_std]
#![no_main]

mod driver;

use apoio_servicos::{ceder, cpl, enviar, receber, resolver, sair};
use contrato_sistema::{
    erro, evento, id_referencia, referencia_valida, InfoServico, ASSINATURA_SERVICO, REDE,
    TERMINAL, VERSAO_ABI,
};
use driver::Rede;

/// Eventos são valores fixos copiados pelo kernel, nunca ponteiros entre processos.
fn notificar(codigo: u64, a: u64, b: u64, c: u64, d: u64) {
    let destino = resolver(TERMINAL);
    if !referencia_valida(destino) {
        return;
    }
    for _ in 0..64 {
        let resultado = enviar(destino, codigo, a, b, c, d);
        if resultado != erro::FILA_CHEIA {
            break;
        }
        ceder();
    }
}

fn informar_erro(codigo: u64) {
    notificar(evento::REDE_ERRO, codigo, 0, 0, 0);
}

#[no_mangle]
#[link_section = ".text.entrada"]
pub extern "sysv64" fn _inicio_servico(informacoes: *const InfoServico) -> ! {
    if informacoes.is_null() || informacoes as usize % core::mem::align_of::<InfoServico>() != 0 {
        sair();
    }
    // SAFETY: página de informações somente leitura construída pelo microkernel.
    let info = unsafe { core::ptr::read_volatile(informacoes) };
    if info.assinatura != ASSINATURA_SERVICO
        || info.versao != VERSAO_ABI
        || info.id != REDE
        || !referencia_valida(info.referencia)
        || id_referencia(info.referencia) != REDE
    {
        sair();
    }
    let mut rede = match unsafe { Rede::inicializar(info.base_fisica) } {
        Ok(rede) => rede,
        Err(_) => {
            notificar(evento::REDE_PRONTA, 1, 0, 0, cpl() as u64);
            sair();
        }
    };
    let estado = rede.estatisticas();
    let mac = estado
        .mac
        .iter()
        .fold(0u64, |numero, &byte| (numero << 8) | byte as u64);
    notificar(
        evento::REDE_PRONTA,
        0,
        mac,
        u32::from_be_bytes(estado.endereco_ipv4) as u64,
        cpl() as u64,
    );
    let mut respostas_notificadas = 0;
    loop {
        // Orçamento de IPC e RX impede que uma única rodada monopolize o serviço.
        for _ in 0..16 {
            let Some(mensagem) = receber() else { break };
            if mensagem.origem != resolver(TERMINAL) {
                continue;
            }
            match mensagem.codigo {
                evento::PING_SOLICITAR => {
                    if mensagem.a > u32::MAX as u64
                        || rede.ping((mensagem.a as u32).to_be_bytes()).is_err()
                    {
                        informar_erro(1);
                    }
                }
                evento::REDE_STATUS_SOLICITAR => {
                    let estado = rede.estatisticas();
                    notificar(
                        evento::REDE_STATUS,
                        estado.recebidos,
                        estado.transmitidos,
                        estado.respostas_ping,
                        estado.ecos_udp,
                    );
                }
                evento::REDE_FALHAR => {
                    // Ensaio solicitado pelo terminal: a falha deve encerrar só este processo.
                    unsafe {
                        core::arch::asm!("ud2", options(noreturn));
                    }
                }
                evento::REDE_ACESSAR_NUCLEO => {
                    // Ensaio de isolamento: esta VA aponta para página supervisor.
                    // A leitura deve causar #PF(14) e encerrar somente este serviço.
                    let _ = unsafe { core::ptr::read_volatile(0x0200_0000 as *const u8) };
                    informar_erro(999); // Acesso indevidamente autorizado, se retornar.
                }
                evento::REDE_OCUPAR => {
                    // Sem syscall CEDER: o PIT precisa preemptar este laço para o terminal viver.
                    notificar(evento::REDE_EM_LACO, 0, 0, 0, 0);
                    loop {
                        core::hint::spin_loop();
                    }
                }
                _ => {}
            }
        }
        rede.consultar();
        let estado = rede.estatisticas();
        if estado.respostas_ping != respostas_notificadas {
            respostas_notificadas = estado.respostas_ping;
            if let Some((destino, sequencia)) = rede.ultimo_ping() {
                notificar(
                    evento::PING_OK,
                    u32::from_be_bytes(destino) as u64,
                    sequencia as u64,
                    0,
                    0,
                );
            }
        }
        if rede.tomar_erro_ping() {
            informar_erro(1);
        }
        ceder();
    }
}

#[panic_handler]
fn panico(_informacao: &core::panic::PanicInfo<'_>) -> ! {
    informar_erro(2);
    sair()
}
