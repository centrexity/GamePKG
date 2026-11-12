.section .multiboot,"a"
.align 4
.long 0x1badb002
.long 0
.long -(0x1badb002)
.section .text
.code32
.global _start
.extern flags_ring0_main
_start:
 cli
 lgdt gdt_pointer
 ljmp $8,$protected
protected:
 mov $16,%ax
 mov %ax,%ds
 mov %ax,%es
 mov %ax,%fs
 mov %ax,%gs
 mov %ax,%ss
 mov $stack_end,%esp
 xor %ebp,%ebp
 call flags_ring0_main
1: hlt
 jmp 1b
.global debug_handler
.extern debug_count
debug_handler:
 incl debug_count
 andl $0xfffffeff,8(%esp)
 iret
.align 8
gdt:
 .quad 0
 .quad 0x00cf9a000000ffff
 .quad 0x00cf92000000ffff
gdt_pointer:
 .word 23
 .long gdt
.section .bss
.align 16
stack_start:
 .skip 65536
stack_end:
.section .note.GNU-stack,"",@progbits
