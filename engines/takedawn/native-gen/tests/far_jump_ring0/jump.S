/* Synthetic ring0/ring3 direct-code far transfer oracle. No player linkage. */
.section .text
.code32
.global far_case,far_target_stub,far_target_stub16,far_gp,far_np,far_bad,far_success
.global kernel_jump32_site,kernel_jump16_site,user_jump32_site,user_jump16_site
.extern resume_sp,resume_ip,seen_vector,seen_error,seen_cs,seen_before,seen_after,seen_ip
.extern far_pointer32,far_pointer16,operand16,user_stack_end
far_case:
 pushfl
 pushal
 mov %esp,resume_sp
 movl $recover,resume_ip
 mov $43,%ax
 mov %ax,%ds
 mov %ax,%es
 mov 40(%esp),%eax
 test %eax,%eax
 jz kernel_jump
 push source_ss(,%eax,4)
 push $user_stack_end
 push $2
 push source_cs(,%eax,4)
 push $user_jump
 iret
kernel_jump:
 cmpl $0,operand16
 jne kernel_jump16
 pushfl
 pop seen_before
kernel_jump32_site:
 ljmp *far_pointer32
kernel_jump16:
 pushfl
 pop seen_before
kernel_jump16_site:
 data16 ljmp *far_pointer16
user_jump:
 cmpl $0,operand16
 jne user_jump16
 pushfl
 pop seen_before
user_jump32_site:
 ljmp *far_pointer32
user_jump16:
 pushfl
 pop seen_before
user_jump16_site:
 data16 ljmp *far_pointer16
user_completed:
 int $128
far_gp:
 movl $13,seen_vector
 jmp fault_common
far_np:
 movl $11,seen_vector
 jmp fault_common
far_bad:
 movl $255,seen_vector
 jmp fault_common
fault_common:
 mov (%esp),%eax
 mov %eax,seen_error
 mov 4(%esp),%eax
 mov %eax,seen_ip
 mov 8(%esp),%eax
 and $65535,%eax
 mov %eax,seen_cs
 mov 12(%esp),%eax
 mov %eax,seen_after
 mov resume_sp,%esp
 jmp *resume_ip
far_success:
 mov resume_sp,%esp
 jmp *resume_ip
recover:
 mov resume_sp,%esp
 popal
 popfl
 ret
/* Fixed VMAs keep table/base observations independent of C compiler layout. */
.section .far_stub32,"ax"
.code32
far_target_stub:
 mov %cs,%ax
 movzwl %ax,%eax
 mov %eax,seen_cs
 pushfl
 pop seen_after
 movl $0,seen_vector
 movl $0,seen_error
 and $3,%eax
 jnz user_return32
 ljmp $8,$recover
user_return32:
 cmp $1,%eax
 je return_ring1_32
 cmp $2,%eax
 je return_ring2_32
 ljmp $35,$user_completed
return_ring1_32:
 ljmp $65,$user_completed
return_ring2_32:
 ljmp $82,$user_completed
.section .far_stub16,"ax"
.code16
far_target_stub16:
 mov %cs,%ax
 movzwl %ax,%eax
 addr32 movl %eax,seen_cs
 pushfl
 addr32 popl seen_after
 addr32 movl $0,seen_vector
 addr32 movl $0,seen_error
 and $3,%eax
 jnz user_return16
 data32 ljmp $8,$recover
user_return16:
 cmp $1,%eax
 je return_ring1_16
 cmp $2,%eax
 je return_ring2_16
 data32 ljmp $35,$user_completed
return_ring1_16:
 data32 ljmp $65,$user_completed
return_ring2_16:
 data32 ljmp $82,$user_completed
.section .rodata
.align 4
source_cs: .long 8,65,82,35
source_ss: .long 16,73,90,43
.section .bss
.align 16
.skip 8192
.global user_stack_end
user_stack_end:
.section .note.GNU-stack,"",@progbits
