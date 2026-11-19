//! Fronteiras de privilegio x86_64: GDT/TSS, IDT, IRQ0 e int 0x80.
//! O ABI deste marco usa apenas registradores inteiros (alvo soft-float).
use core::{
    arch::{asm, global_asm},
    mem,
    ptr::{addr_of, addr_of_mut, write, write_unaligned},
};

#[repr(C, packed)]
struct PonteiroTabela {
    limite: u16,
    endereco: u64,
}
static mut GDT: [u64; 7] = [
    0,
    0x00af_9a00_0000_ffff,
    0x00cf_9200_0000_ffff,
    0x00cf_f200_0000_ffff,
    0x00af_fa00_0000_ffff,
    0,
    0,
];

#[repr(C, packed)]
struct Tss {
    reservado0: u32,
    rsp0: u64,
    rsp1: u64,
    rsp2: u64,
    reservado1: u64,
    ist: [u64; 7],
    reservado2: u64,
    reservado3: u16,
    mapa_io: u16,
}
#[repr(C, packed)]
struct TssIo {
    tss: Tss,
    bitmap: [u8; 8193],
}
#[repr(C, align(16))]
struct TarefaHardware(TssIo);
static mut TSS: TarefaHardware = TarefaHardware(TssIo {
    tss: Tss {
        reservado0: 0,
        rsp0: 0,
        rsp1: 0,
        rsp2: 0,
        reservado1: 0,
        ist: [0; 7],
        reservado2: 0,
        reservado3: 0,
        mapa_io: 104,
    },
    bitmap: [0xff; 8193],
});
#[repr(C, align(4096))]
struct PilhaFalha([u8; 16384]);
static mut PILHA_FALHA: PilhaFalha = PilhaFalha([0; 16384]);

#[repr(C)]
#[derive(Clone, Copy)]
struct PortaIdt {
    baixo: u16,
    seletor: u16,
    ist: u8,
    atributos: u8,
    meio: u16,
    alto: u32,
    reservado: u32,
}
impl PortaIdt {
    const VAZIA: Self = Self {
        baixo: 0,
        seletor: 0,
        ist: 0,
        atributos: 0,
        meio: 0,
        alto: 0,
        reservado: 0,
    };
    fn nova(endereco: u64, atributos: u8, ist: u8) -> Self {
        Self {
            baixo: endereco as u16,
            seletor: 8,
            ist,
            atributos,
            meio: (endereco >> 16) as u16,
            alto: (endereco >> 32) as u32,
            reservado: 0,
        }
    }
}
#[repr(C, align(16))]
struct Idt([PortaIdt; 256]);
static mut IDT: Idt = Idt([PortaIdt::VAZIA; 256]);

/// Ordem exata produzida por entrada_comum; 176 bytes.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Quadro {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rbp: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,
    pub vetor: u64,
    pub erro: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}
extern "C" {
    static VETORES_EXCECAO: [u64; 32];
    fn entrada_irq0();
    fn entrada_irq4();
    fn entrada_syscall();
    fn entrada_inesperada();
    fn restaurar_contexto(quadro: *const Quadro) -> !;
}

pub unsafe fn iniciar() {
    asm!("cli", options(nomem, nostack));
    // NX e obrigatorio: nunca continuar silenciosamente com dados executaveis.
    let max = core::arch::x86_64::__cpuid(0x8000_0000).eax;
    if max < 0x8000_0001 || core::arch::x86_64::__cpuid(0x8000_0001).edx & (1 << 20) == 0 {
        crate::io::serial_texto("[microkernel] CPU sem NX: plataforma recusada.\n");
        crate::io::parar();
    }
    let lo: u32;
    let hi: u32;
    asm!("rdmsr",in("ecx")0xc000_0080u32,out("eax")lo,out("edx")hi,options(nostack));
    asm!("wrmsr",in("ecx")0xc000_0080u32,in("eax")lo|(1<<11),in("edx")hi,options(nostack));
    let mut cr0: u64;
    asm!("mov {}, cr0",out(reg)cr0,options(nostack));
    // WP protege paginas readonly tambem de escritas acidentais privilegiadas.
    // TS faz uso de FPU/SIMD nao previsto falhar, em vez de compartilhar seu estado.
    cr0 |= (1 << 16) | (1 << 3);
    asm!("mov cr0, {}",in(reg)cr0,options(nostack));
    let tss = addr_of!(TSS.0) as u64;
    let limite = (mem::size_of::<TssIo>() - 1) as u64;
    let gdt = addr_of_mut!(GDT).cast::<u64>();
    write(
        gdt.add(5),
        (limite & 0xffff)
            | ((tss & 0xffff) << 16)
            | (((tss >> 16) & 0xff) << 32)
            | (0x89 << 40)
            | (((limite >> 16) & 15) << 48)
            | (((tss >> 24) & 0xff) << 56),
    );
    write(gdt.add(6), tss >> 32);
    write_unaligned(
        addr_of_mut!(TSS.0.tss.ist).cast::<u64>(),
        addr_of!(PILHA_FALHA.0) as u64 + 16384,
    );
    let p = PonteiroTabela {
        limite: (mem::size_of::<[u64; 7]>() - 1) as u16,
        endereco: gdt as u64,
    };
    asm!("lgdt [{}]",in(reg)&p,options(readonly,nostack));
    asm!("push 0x08","lea rax, [rip + 2f]","push rax","retfq","2:",
        "mov ax, 0x10","mov ds, ax","mov es, ax","mov ss, ax","xor eax, eax","mov fs, ax","mov gs, ax",out("rax")_);
    asm!("mov ax, 0x28","ltr ax",out("rax")_,options(nostack));
    let idt = addr_of_mut!(IDT.0).cast::<PortaIdt>();
    for n in 0..256 {
        let endereco = if n < 32 {
            VETORES_EXCECAO[n]
        } else {
            entrada_inesperada as *const () as u64
        };
        write(
            idt.add(n),
            PortaIdt::nova(endereco, 0x8e, if n == 8 { 1 } else { 0 }),
        );
    }
    write(
        idt.add(32),
        PortaIdt::nova(entrada_irq0 as *const () as u64, 0x8e, 0),
    );
    write(
        idt.add(36),
        PortaIdt::nova(entrada_irq4 as *const () as u64, 0x8e, 0),
    );
    // Unica porta invocavel por CPL3. Interrupt gate mantem o kernel nao reentrante.
    write(
        idt.add(128),
        PortaIdt::nova(entrada_syscall as *const () as u64, 0xee, 0),
    );
    let p = PonteiroTabela {
        limite: (mem::size_of::<Idt>() - 1) as u16,
        endereco: idt as u64,
    };
    asm!("lidt [{}]",in(reg)&p,options(readonly,nostack));
    iniciar_temporizador();
}

unsafe fn esperar_io() {
    crate::io::saida8(0x80, 0);
}
unsafe fn iniciar_temporizador() {
    use crate::io::saida8;
    saida8(0x21, 0xff);
    saida8(0xa1, 0xff);
    saida8(0x20, 0x11);
    esperar_io();
    saida8(0xa0, 0x11);
    esperar_io();
    saida8(0x21, 0x20);
    esperar_io();
    saida8(0xa1, 0x28);
    esperar_io();
    saida8(0x21, 4);
    esperar_io();
    saida8(0xa1, 2);
    esperar_io();
    saida8(0x21, 1);
    esperar_io();
    saida8(0xa1, 1);
    esperar_io();
    // PIT canal 0, modo 3, aproximadamente 100 Hz.
    let divisor = 1193182u32 / 100;
    saida8(0x43, 0x36);
    saida8(0x40, divisor as u8);
    saida8(0x40, (divisor >> 8) as u8);
    saida8(0x21, 0xfe);
    saida8(0xa1, 0xff);
}

/// Executado somente com IF=0, antes de retornar ao processo escolhido.
pub unsafe fn selecionar(pilha_kernel: u64, terminal: bool, cr3: u64) {
    write_unaligned(addr_of_mut!(TSS.0.tss.rsp0), pilha_kernel);
    let mapa = addr_of_mut!(TSS.0.bitmap).cast::<u8>();
    core::ptr::write_bytes(mapa, 0xff, 8193);
    if terminal {
        for porta in 0x3f8usize..=0x3ff {
            let p = mapa.add(porta / 8);
            *p &= !(1 << (porta % 8));
        }
        for porta in [0x60usize, 0x64] {
            let p = mapa.add(porta / 8);
            *p &= !(1 << (porta % 8));
        }
    }
    asm!("mov cr3, {}",in(reg)cr3,options(nostack));
}
pub unsafe fn executar(quadro: *const Quadro) -> ! {
    restaurar_contexto(quadro)
}
pub fn cr3() -> u64 {
    let v;
    unsafe {
        asm!("mov {}, cr3",out(reg)v,options(nomem,nostack,preserves_flags));
    }
    v
}
pub fn cr2() -> u64 {
    let v;
    unsafe {
        asm!("mov {}, cr2",out(reg)v,options(nomem,nostack,preserves_flags));
    }
    v
}
/// IF=0; preserva todas as demais linhas do PIC, inclusive IRQ0.
pub unsafe fn mascara_irq4(mascarar: bool) {
    let atual = crate::io::entrada8(0x21);
    crate::io::saida8(
        0x21,
        if mascarar {
            atual | 0x10
        } else {
            atual & !0x10
        },
    );
}
pub unsafe fn confirmar_irq0() {
    crate::io::saida8(0x20, 0x20);
}

global_asm!(
    r#"
.section .text
.macro SEM_ERRO numero
.global falha_\numero
falha_\numero:
    push 0
    push \numero
    jmp entrada_comum
.endm
.macro COM_ERRO numero
.global falha_\numero
falha_\numero:
    push \numero
    jmp entrada_comum
.endm
SEM_ERRO 0
SEM_ERRO 1
SEM_ERRO 2
SEM_ERRO 3
SEM_ERRO 4
SEM_ERRO 5
SEM_ERRO 6
SEM_ERRO 7
COM_ERRO 8
SEM_ERRO 9
COM_ERRO 10
COM_ERRO 11
COM_ERRO 12
COM_ERRO 13
COM_ERRO 14
SEM_ERRO 15
SEM_ERRO 16
COM_ERRO 17
SEM_ERRO 18
SEM_ERRO 19
SEM_ERRO 20
COM_ERRO 21
SEM_ERRO 22
SEM_ERRO 23
SEM_ERRO 24
SEM_ERRO 25
SEM_ERRO 26
SEM_ERRO 27
SEM_ERRO 28
COM_ERRO 29
COM_ERRO 30
SEM_ERRO 31
.global entrada_irq0
entrada_irq0:
    push 0
    push 32
    jmp entrada_comum
.global entrada_irq4
entrada_irq4:
    push 0
    push 36
    jmp entrada_comum
.global entrada_syscall
entrada_syscall:
    push 0
    push 128
    jmp entrada_comum
.global entrada_inesperada
entrada_inesperada:
    push 0
    push 255
entrada_comum:
    cld
    push rax
    push rbx
    push rcx
    push rdx
    push rsi
    push rdi
    push rbp
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15
    mov rdi, rsp
    and rsp, -16
    call despachar_interrupcao
    mov rsp, rax
    jmp retorno_contexto
.global restaurar_contexto
restaurar_contexto:
    mov rsp, rdi
retorno_contexto:
    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    pop rbp
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rbx
    pop rax
    add rsp, 16
    iretq
.section .rodata
.balign 8
.global VETORES_EXCECAO
VETORES_EXCECAO:
.quad falha_0, falha_1, falha_2, falha_3, falha_4, falha_5, falha_6, falha_7
.quad falha_8, falha_9, falha_10, falha_11, falha_12, falha_13, falha_14, falha_15
.quad falha_16, falha_17, falha_18, falha_19, falha_20, falha_21, falha_22, falha_23
.quad falha_24, falha_25, falha_26, falha_27, falha_28, falha_29, falha_30, falha_31
"#
);
