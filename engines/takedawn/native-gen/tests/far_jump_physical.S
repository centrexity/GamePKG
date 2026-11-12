/* ELF32-only synthetic far-transfer probe, entirely within this process. */
.text
.code32
.global far_physical_case
.global far_physical_jump_site
.type far_physical_case,@function
far_physical_case:
 pushfl
 pushal
 mov %esp,far_physical_stack
 movl $far_physical_resume,far_physical_return
 mov %cs,%ax
 movzwl %ax,%eax
 mov %eax,far_physical_cs
 mov 40(%esp),%edx
 mov $0x11223344,%eax
far_physical_jump_site:
 ljmp *(%edx)
far_physical_resume:
 popal
 popfl
 ret
.size far_physical_case,.-far_physical_case
.section .note.GNU-stack,"",@progbits
