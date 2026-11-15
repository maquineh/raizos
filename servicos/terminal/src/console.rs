use contrato_boot::Tela;
use core::{
    cell::UnsafeCell,
    fmt::{self, Write},
    ptr,
};

struct Exclusivo<T>(UnsafeCell<T>);
// Invariante: uma thread neste processo. A preempcao troca de processo sem
// reentrar neste console. Excecoes sao tratadas pelo microkernel em ring 0.
unsafe impl<T> Sync for Exclusivo<T> {}
static CONSOLE: Exclusivo<Option<Console>> = Exclusivo(UnsafeCell::new(None));
const FUNDO: u32 = 0x101b25;
const TEXTO: u32 = 0xd3e8e3;
const ACENTO: u32 = 0x55d6b2;
const ALTURA_CELULA: usize = 16;
const LARGURA_CELULA: usize = 8;

struct Console {
    tela: Tela,
    coluna: usize,
    linha: usize,
    colunas: usize,
    linhas: usize,
}

pub unsafe fn iniciar(tela: Tela) {
    let mut c = Console {
        tela,
        coluna: 0,
        linha: 1,
        colunas: (tela.largura as usize / LARGURA_CELULA).saturating_sub(2),
        linhas: (tela.altura as usize / ALTURA_CELULA).saturating_sub(1),
    };
    if c.colunas < 20 || c.linhas < 5 {
        c.tela.endereco = 0;
    }
    c.limpar();
    *CONSOLE.0.get() = Some(c);
}

pub fn escrever(args: fmt::Arguments<'_>) {
    unsafe {
        if let Some(c) = (&mut *CONSOLE.0.get()).as_mut() {
            let _ = c.write_fmt(args);
        } else {
            let _ = crate::io::Emergencia.write_fmt(args);
        }
    }
}
pub fn limpar() {
    crate::io::serial_texto("\x1b[2J\x1b[H");
    unsafe {
        if let Some(c) = (&mut *CONSOLE.0.get()).as_mut() {
            c.limpar();
        }
    }
}

impl Console {
    fn pixel(&self, x: usize, y: usize, cor: u32) {
        if self.tela.endereco == 0
            || x >= self.tela.largura as usize
            || y >= self.tela.altura as usize
        {
            return;
        }
        let cor = if self.tela.formato == 0 {
            (cor & 0x00ff00) | ((cor & 0xff) << 16) | ((cor >> 16) & 0xff)
        } else {
            cor
        };
        let i = y * self.tela.pixels_por_linha as usize + x;
        unsafe {
            ptr::write_volatile((self.tela.endereco as *mut u32).add(i), cor);
        }
    }
    fn limpar(&mut self) {
        for y in 0..self.tela.altura as usize {
            for x in 0..self.tela.largura as usize {
                self.pixel(x, y, FUNDO);
            }
        }
        for y in 0..3 {
            for x in 0..self.tela.largura as usize {
                self.pixel(x, y, ACENTO);
            }
        }
        self.coluna = 0;
        self.linha = 1;
    }
    fn proxima_linha(&mut self) {
        self.coluna = 0;
        self.linha += 1;
        if self.linha >= self.linhas {
            if self.tela.endereco != 0 {
                let p = self.tela.endereco as *mut u32;
                let stride = self.tela.pixels_por_linha as usize;
                for y in ALTURA_CELULA..(self.linhas - 1) * ALTURA_CELULA {
                    for x in 0..self.tela.largura as usize {
                        unsafe {
                            ptr::write_volatile(
                                p.add(y * stride + x),
                                ptr::read_volatile(p.add((y + ALTURA_CELULA) * stride + x)),
                            );
                        }
                    }
                }
                for y in (self.linhas - 1) * ALTURA_CELULA..self.linhas * ALTURA_CELULA {
                    for x in 0..self.tela.largura as usize {
                        self.pixel(x, y, FUNDO);
                    }
                }
            }
            self.linha = self.linhas.saturating_sub(1);
        }
    }
    fn desenhar(&self, c: char) {
        let forma = crate::fonte::glifo(c);
        let ox = (self.coluna + 1) * LARGURA_CELULA;
        let oy = self.linha * ALTURA_CELULA;
        for y in 0..ALTURA_CELULA {
            for x in 0..LARGURA_CELULA {
                let aceso = x < 5 && y < 14 && (forma[y / 2] & (1 << (4 - x))) != 0;
                self.pixel(ox + x, oy + y, if aceso { TEXTO } else { FUNDO });
            }
        }
    }
}
impl Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        crate::io::serial_texto(s);
        for c in s.chars() {
            match c {
                '\n' => self.proxima_linha(),
                '\r' => self.coluna = 0,
                '\x08' => {
                    if self.coluna > 0 {
                        self.coluna -= 1;
                        self.desenhar(' ');
                    }
                }
                c if c.is_control() => {}
                _ => {
                    if self.coluna >= self.colunas {
                        self.proxima_linha();
                    }
                    self.desenhar(c);
                    self.coluna += 1;
                }
            }
        }
        Ok(())
    }
}

#[macro_export]
macro_rules! imprimir { ($($arg:tt)*) => { $crate::console::escrever(format_args!($($arg)*)) }; }
#[macro_export]
macro_rules! imprimirln {
    () => { $crate::imprimir!("\n") };
    ($($arg:tt)*) => { $crate::imprimir!("{}\n", format_args!($($arg)*)) };
}
