//! Teclado PS/2 set 1, layout US, sem interrupcoes. Serial funciona em paralelo.
use crate::io::{entrada8, saida8};
pub struct Teclado {
    shift: bool,
    estendido: bool,
}
impl Teclado {
    pub unsafe fn iniciar() -> Self {
        for _ in 0..100 {
            if entrada8(0x64) & 1 == 0 {
                break;
            }
            let _ = entrada8(0x60);
        }
        if esperar() {
            saida8(0x64, 0x20);
        }
        let mut config = None;
        for _ in 0..100_000 {
            if entrada8(0x64) & 1 != 0 {
                config = Some(entrada8(0x60));
                break;
            }
        }
        if let Some(c) = config {
            if esperar() {
                saida8(0x64, 0x60);
            }
            if esperar() {
                saida8(0x60, (c | 0x40) & !0x13);
            }
        }
        if esperar() {
            saida8(0x64, 0xae);
        }
        if esperar() {
            saida8(0x60, 0xf4);
        }
        Self {
            shift: false,
            estendido: false,
        }
    }
    pub fn ler(&mut self) -> Option<u8> {
        let estado = unsafe { entrada8(0x64) };
        if estado & 1 == 0 {
            return None;
        }
        let c = unsafe { entrada8(0x60) };
        if estado & 0x20 != 0 {
            return None;
        } // mouse
        if c == 0xe0 {
            self.estendido = true;
            return None;
        }
        if self.estendido {
            self.estendido = false;
            return None;
        }
        if c == 0x2a || c == 0x36 {
            self.shift = true;
            return None;
        }
        if c == 0xaa || c == 0xb6 {
            self.shift = false;
            return None;
        }
        if c & 0x80 != 0 {
            return None;
        }
        let normal = b"\0\x1b1234567890-=\x08\tqwertyuiop[]\n\0asdfghjkl;'`\0\\zxcvbnm,./\0*\0 ";
        let shift = b"\0\x1b!@#$%^&*()_+\x08\tQWERTYUIOP{}\n\0ASDFGHJKL:\"~\0|ZXCVBNM<>?\0*\0 ";
        let b = *(if self.shift { shift } else { normal }).get(c as usize)?;
        if b == 0 {
            None
        } else {
            Some(b)
        }
    }
}
unsafe fn esperar() -> bool {
    for _ in 0..100_000 {
        if entrada8(0x64) & 2 == 0 {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}
