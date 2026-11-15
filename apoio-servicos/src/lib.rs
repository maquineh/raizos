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
