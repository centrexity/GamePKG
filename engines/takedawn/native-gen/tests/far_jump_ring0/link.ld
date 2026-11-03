ENTRY(_start)
PHDRS { text PT_LOAD FLAGS(5); data PT_LOAD FLAGS(6); }
SECTIONS {
  . = 0x100000;
  .multiboot : { KEEP(*(.multiboot)) } :text
  .text : { *(.text*) } :text
  .rodata : { *(.rodata*) } :text
  .far_stub32 0x200000 : { *(.far_stub32) } :text
  .far_stub16 0x210000 : { *(.far_stub16) } :text
  . = ALIGN(4096);
  .data : { *(.data*) } :data
  .bss : { *(.bss*) *(COMMON) } :data
  /DISCARD/ : { *(.eh_frame*) *(.comment*) }
}
