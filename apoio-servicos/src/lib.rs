#![no_std]
use contrato_sistema::{chamada, referencia_valida, Mensagem};
use core::arch::asm;

#[inline]
pub fn chamar(numero: u64, a: u64, b: u64, c: u64, d: u64, e: u64, f: u64) -> u64 {
    let resultado;
    // A ABI preserva os demais registradores; recebem resultado somente os listados.
    // int80 pode alternar processos e tem efeitos de memoria: nao usar nomem/pure.
    unsafe {
        asm!("int 0x80",inlateout("rax")numero=>resultado,
        inlateout("rdi")a=>_,inlateout("rsi")b=>_,inlateout("rdx")c=>_,
        inlateout("r10")d=>_,inlateout("r8")e=>_,inlateout("r9")f=>_);
    }
    resultado
}
pub fn ceder() {
    chamar(chamada::CEDER, 0, 0, 0, 0, 0, 0);
}
/// Espera atividade IPC na caixa propria; confirme o resultado com receber().
/// Kernels ABI 2 anteriores podem retornar erro::CHAMADA.
pub fn esperar_ipc() -> u64 {
    chamar(chamada::ESPERAR_IPC, 0, 0, 0, 0, 0, 0)
}
pub fn enviar(destino: u64, codigo: u64, a: u64, b: u64, c: u64, d: u64) -> u64 {
    chamar(chamada::ENVIAR, destino, codigo, a, b, c, d)
}
pub fn receber() -> Option<Mensagem> {
    let (codigo, origem, a, b, c, d);
    unsafe {
        asm!("int 0x80",inlateout("rax")chamada::RECEBER=>codigo,
        lateout("rdi")origem,lateout("rsi")a,lateout("rdx")b,
        lateout("r10")c,lateout("r8")d,lateout("r9")_);
    }
    if codigo == 0 {
        None
    } else {
        Some(Mensagem {
            origem,
            codigo,
            a,
            b,
            c,
            d,
        })
    }
}
/// Recebe uma mensagem ou um erro. Kernels ABI 2 anteriores retornam CHAMADA.
pub fn receber_bloqueante() -> Result<Mensagem, u64> {
    let (resultado, origem, codigo, a, b, c, d);
    unsafe {
        asm!("int 0x80", inlateout("rax") chamada::RECEBER_BLOQUEANTE => resultado,
            lateout("rdi") origem, lateout("rsi") codigo, lateout("rdx") a,
            lateout("r10") b, lateout("r8") c, lateout("r9") d);
    }
    if resultado != 0 {
        Err(resultado)
    } else {
        Ok(Mensagem {
            origem,
            codigo,
            a,
            b,
            c,
            d,
        })
    }
}
/// Modo e duracao seguem contrato_sistema::prazo; CHAMADA indica indisponibilidade
/// em kernels ABI 2 anteriores ou argumentos invalidos.
pub fn receber_com_prazo(modo: u64, duracao: u64) -> Result<Mensagem, u64> {
    let (resultado, origem, codigo, a, b, c, d);
    unsafe {
        asm!("int 0x80", inlateout("rax") chamada::RECEBER_COM_PRAZO => resultado,
            inlateout("rdi") modo => origem, inlateout("rsi") duracao => codigo,
            lateout("rdx") a, lateout("r10") b, lateout("r8") c, lateout("r9") d);
    }
    if resultado != 0 {
        Err(resultado)
    } else {
        Ok(Mensagem {
            origem,
            codigo,
            a,
            b,
            c,
            d,
        })
    }
}
/// Apenas supervisor: aguarda IPC proprio ou indisponibilidade da rede exata.
pub fn receber_observando(alvo: u64, modo: u64, duracao: u64) -> Result<Mensagem, u64> {
    let (resultado, origem, codigo, a, b, c, d);
    unsafe {
        asm!("int 0x80", inlateout("rax") chamada::RECEBER_OBSERVANDO => resultado,
            inlateout("rdi") alvo => origem, inlateout("rsi") modo => codigo,
            inlateout("rdx") duracao => a, lateout("r10") b, lateout("r8") c, lateout("r9") d);
    }
    if resultado != 0 {
        Err(resultado)
    } else {
        Ok(Mensagem {
            origem,
            codigo,
            a,
            b,
            c,
            d,
        })
    }
}
pub enum EventoSistema {
    Ipc(Mensagem),
    Irq {
        sequencia: u64,
    },
    Vida {
        referencia: u64,
        tipo: u64,
        estado: u64,
    },
}
pub fn armar_irq4() -> u64 {
    chamar(chamada::ARMAR_IRQ4, 0, 0, 0, 0, 0, 0)
}
pub fn confirmar_irq4(sequencia: u64) -> u64 {
    chamar(chamada::CONFIRMAR_IRQ4, sequencia, 0, 0, 0, 0, 0)
}
pub fn esperar_irq_ipc(modo: u64, duracao: u64) -> Result<EventoSistema, u64> {
    let (classe, rdi, rsi, rdx, r10, r8, r9);
    unsafe {
        asm!("int 0x80", inlateout("rax") chamada::ESPERAR_IRQ_IPC => classe,
        inlateout("rdi") modo => rdi, inlateout("rsi") duracao => rsi,
        lateout("rdx") rdx, lateout("r10") r10, lateout("r8") r8, lateout("r9") r9);
    }
    match classe {
        contrato_sistema::classe_evento::IRQ => Ok(EventoSistema::Irq { sequencia: rdi }),
        contrato_sistema::classe_evento::IPC => Ok(EventoSistema::Ipc(Mensagem {
            origem: rdi,
            codigo: rsi,
            a: rdx,
            b: r10,
            c: r8,
            d: r9,
        })),
        e => Err(e),
    }
}
pub fn inscrever_vida(referencia: u64) -> Result<u64, u64> {
    let (status, estado);
    unsafe {
        asm!("int 0x80", inlateout("rax") chamada::INSCREVER_VIDA => status,
        inlateout("rdi") referencia => estado, lateout("rsi") _, lateout("rdx") _,
        lateout("r10") _, lateout("r8") _, lateout("r9") _);
    }
    if status == 0 {
        Ok(estado)
    } else {
        Err(status)
    }
}
pub fn cancelar_vida(referencia: u64) -> u64 {
    chamar(chamada::CANCELAR_VIDA, referencia, 0, 0, 0, 0, 0)
}
pub fn esperar_evento(modo: u64, duracao: u64) -> Result<EventoSistema, u64> {
    let (classe, rdi, rsi, rdx, r10, r8, r9);
    unsafe {
        asm!("int 0x80", inlateout("rax") chamada::ESPERAR_EVENTO => classe,
        inlateout("rdi") modo => rdi, inlateout("rsi") duracao => rsi,
        lateout("rdx") rdx, lateout("r10") r10, lateout("r8") r8, lateout("r9") r9);
    }
    match classe {
        contrato_sistema::classe_evento::IPC => Ok(EventoSistema::Ipc(Mensagem {
            origem: rdi,
            codigo: rsi,
            a: rdx,
            b: r10,
            c: r8,
            d: r9,
        })),
        contrato_sistema::classe_evento::VIDA => Ok(EventoSistema::Vida {
            referencia: rdi,
            tipo: rsi,
            estado: rdx,
        }),
        e => Err(e),
    }
}
pub fn estado(id: u64) -> u64 {
    let referencia = resolver(id);
    if referencia_valida(referencia) {
        estado_referencia(referencia)
    } else {
        contrato_sistema::estado::INEXISTENTE
    }
}
/// Consulta uma instancia exata; uma referencia de geracao anterior e recusada.
pub fn estado_referencia(referencia: u64) -> u64 {
    chamar(chamada::ESTADO_SERVICO, referencia, 0, 0, 0, 0, 0)
}
pub fn resolver(id: u64) -> u64 {
    chamar(chamada::RESOLVER, id, 0, 0, 0, 0, 0)
}
pub fn reiniciar(referencia: u64) -> u64 {
    chamar(chamada::REINICIAR, referencia, 0, 0, 0, 0, 0)
}
pub fn encerrar(referencia: u64) -> u64 {
    chamar(chamada::ENCERRAR, referencia, 0, 0, 0, 0, 0)
}
pub fn tiques() -> u64 {
    chamar(chamada::TIQUES, 0, 0, 0, 0, 0, 0)
}
pub fn sair() -> ! {
    chamar(chamada::SAIR, 0, 0, 0, 0, 0, 0);
    loop {
        core::hint::spin_loop();
    }
}
pub fn cpl() -> u16 {
    let cs: u16;
    unsafe {
        asm!("mov {0:x}, cs",out(reg)cs,options(nomem,nostack,preserves_flags));
    }
    cs & 3
}
