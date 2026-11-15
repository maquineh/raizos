ENTRY(_inicio_servico)
PHDRS { texto PT_LOAD FLAGS(5); leitura PT_LOAD FLAGS(4); dados PT_LOAD FLAGS(6); }
SECTIONS {
  . = 0x40000000;
  .text ALIGN(4096) : { KEEP(*(.text.entrada)) *(.text .text.*) } :texto
  .rodata ALIGN(4096) : { *(.rodata .rodata.*) } :leitura
  .data ALIGN(4096) : { *(.data .data.*) *(.got .got.*) } :dados
  .bss ALIGN(4096) (NOLOAD) : { *(.bss .bss.*) *(COMMON) } :dados
  . = ALIGN(4096);
  ASSERT(. < 0x401d0000, "Servico ultrapassou janela de imagem")
  /DISCARD/ : { *(.eh_frame*) *(.comment*) *(.note*) }
}

