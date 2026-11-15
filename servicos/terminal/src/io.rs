//! Acesso x86 às portas de E/S. Restrito ao alvo emulado e ao modo privilegiado.
use core::arch::asm;

pub unsafe fn saida8(porta: u16, valor: u8) {
    asm!("out dx, al", in("dx") porta, in("al") valor, options(nomem, nostack, preserves_flags));
}
pub unsafe fn entrada8(porta: u16) -> u8 {
    let v;
    asm!("in al, dx", in("dx") porta, out("al") v, options(nomem, nostack, preserves_flags));
    v
}

pub unsafe fn iniciar_serial() {
    saida8(0x3f9, 0);
    saida8(0x3fb, 0x80);
    saida8(0x3f8, 1);
    saida8(0x3f9, 0); // 115200, 8N1
    saida8(0x3fb, 3);
    saida8(0x3fa, 0xc7);
    saida8(0x3fc, 3);
}

pub fn byte_serial(b: u8) {
    for _ in 0..100_000 {
        if unsafe { entrada8(0x3fd) } & 0x20 != 0 {
            unsafe {
                saida8(0x3f8, b);
            }
            return;
        }
        core::hint::spin_loop();
    }
}

pub fn ler_serial() -> Option<u8> {
    if unsafe { entrada8(0x3fd) } & 1 != 0 {
        Some(unsafe { entrada8(0x3f8) })
    } else {
        None
    }
}

pub fn serial_texto(s: &str) {
    for b in s.bytes() {
        if b == b'\n' {
            byte_serial(b'\r');
        }
        byte_serial(b);
    }
}

pub struct Emergencia;
impl core::fmt::Write for Emergencia {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        serial_texto(s);
        Ok(())
    }
}
