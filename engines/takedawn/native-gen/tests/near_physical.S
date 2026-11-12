/* ELF32 maintainer-only branch/stack probes within a process-local LDT. */
.text
.code32
.global near_physical_case
.type near_physical_case,@function
near_physical_case:
 pushfl
 pushal
 mov %esp,near_physical_stack
 movl $near_physical_resume,near_physical_return
 mov %cs,%ax
 movzwl %ax,%eax
 mov %eax,near_physical_cs
 mov %ss,%ax
 movzwl %ax,%eax
 mov %eax,near_physical_ss
 mov 40(%esp),%ebp
 mov 44(%esp),%edx
 mov 48(%esp),%ecx
 mov 52(%esp),%eax
 push %eax
 popfl
 pushfl
 popl near_physical_flags
 mov 56(%esp),%esi
 mov 60(%esp),%edi
 mov %edx,%esp
 mov $0x11223344,%eax
 ljmp *(%ebp)
near_physical_resume:
 popal
 popfl
 ret
.size near_physical_case,.-near_physical_case
.section .note.GNU-stack,"",@progbits
