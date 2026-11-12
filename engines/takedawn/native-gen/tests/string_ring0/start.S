.section .multiboot,"a"
.align 4
.long 0x1badb002,0,-0x1badb002
.section .text
.code32
.global _start
.extern string_ring0_main
_start:
 cli
 lgdt gdt_pointer
 ljmp $8,$protected
protected:
 mov $16,%ax
 mov %ax,%ds
 mov %ax,%es
 mov %ax,%gs
 mov %ax,%ss
 mov $24,%ax
 mov %ax,%fs
 mov $stack_end,%esp
 xor %ebp,%ebp
 call string_ring0_main
1: hlt
 jmp 1b
.align 8
gdt:
 .quad 0,0x00cf9a000000ffff,0x00cf92000000ffff,0x00cf92002000ffff
gdt_pointer:
 .word 31
 .long gdt
.section .bss
.align 16
 .skip 65536
stack_end:
.section .note.GNU-stack,"",@progbits
