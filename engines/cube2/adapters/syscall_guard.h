// GamePKG host adaptation, Zlib license; see ../upstream/readme_source.txt.
// Mandatory Linux guard for the reviewed software-rendered Cube 2 engine.
// Filesystem scope comes from mandatory mount namespaces; this filter permits
// software rendering and threads, but denies process creation/exec and sockets.
#ifndef __linux__
#error Cube_2_requires_a_supported_GamePKG_runtime_guard
#endif
#include <linux/audit.h>
#include <linux/filter.h>
#include <linux/seccomp.h>
#include <sys/prctl.h>
#include <sys/syscall.h>
#include <stddef.h>
#include <errno.h>
#include <unistd.h>
#include <stdlib.h>
#include <stdint.h>

#if defined(__x86_64__)
#define GAMEPKG_AUDIT_ARCH AUDIT_ARCH_X86_64
#elif defined(__i386__)
#define GAMEPKG_AUDIT_ARCH AUDIT_ARCH_I386
#elif defined(__aarch64__)
#define GAMEPKG_AUDIT_ARCH AUDIT_ARCH_AARCH64
#elif defined(__arm__)
#define GAMEPKG_AUDIT_ARCH AUDIT_ARCH_ARM
#else
#error Unsupported_GamePKG_Cube_2_syscall_architecture
#endif

static void gamepkg_lockdown()
{
    static const uint32_t allowed[] = {
#ifdef SYS_close_range
        SYS_close_range,
#endif
#ifdef SYS_read
        SYS_read,
#endif
#ifdef SYS_write
        SYS_write,
#endif
#ifdef SYS_close
        SYS_close,
#endif
#ifdef SYS_readv
        SYS_readv,
#endif
#ifdef SYS_writev
        SYS_writev,
#endif
#ifdef SYS_fstat
        SYS_fstat,
#endif
#ifdef SYS_newfstatat
        SYS_newfstatat,
#endif
#ifdef SYS_statx
        SYS_statx,
#endif
#ifdef SYS_stat
        SYS_stat,
#endif
#ifdef SYS_lstat
        SYS_lstat,
#endif
#ifdef SYS_access
        SYS_access,
#endif
#ifdef SYS_faccessat
        SYS_faccessat,
#endif
#ifdef SYS_faccessat2
        SYS_faccessat2,
#endif
#ifdef SYS_fcntl
        SYS_fcntl,
#endif
#ifdef SYS_fcntl64
        SYS_fcntl64,
#endif
#ifdef SYS_lseek
        SYS_lseek,
#endif
#ifdef SYS__llseek
        SYS__llseek,
#endif
#ifdef SYS_pread64
        SYS_pread64,
#endif
#ifdef SYS_pwrite64
        SYS_pwrite64,
#endif
#ifdef SYS_getdents
        SYS_getdents,
#endif
#ifdef SYS_getdents64
        SYS_getdents64,
#endif
#ifdef SYS_open
        SYS_open,
#endif
#ifdef SYS_openat
        SYS_openat,
#endif
#ifdef SYS_getcwd
        SYS_getcwd,
#endif
#ifdef SYS_chdir
        SYS_chdir,
#endif
#ifdef SYS_fchdir
        SYS_fchdir,
#endif
#ifdef SYS_readlink
        SYS_readlink,
#endif
#ifdef SYS_readlinkat
        SYS_readlinkat,
#endif
#ifdef SYS_mkdir
        SYS_mkdir,
#endif
#ifdef SYS_mkdirat
        SYS_mkdirat,
#endif
#ifdef SYS_rmdir
        SYS_rmdir,
#endif
#ifdef SYS_unlink
        SYS_unlink,
#endif
#ifdef SYS_unlinkat
        SYS_unlinkat,
#endif
#ifdef SYS_rename
        SYS_rename,
#endif
#ifdef SYS_renameat
        SYS_renameat,
#endif
#ifdef SYS_renameat2
        SYS_renameat2,
#endif
#ifdef SYS_fsync
        SYS_fsync,
#endif
#ifdef SYS_fdatasync
        SYS_fdatasync,
#endif
#ifdef SYS_ftruncate
        SYS_ftruncate,
#endif
#ifdef SYS_ftruncate64
        SYS_ftruncate64,
#endif
#ifdef SYS_truncate
        SYS_truncate,
#endif
#ifdef SYS_truncate64
        SYS_truncate64,
#endif
#ifdef SYS_statfs
        SYS_statfs,
#endif
#ifdef SYS_statfs64
        SYS_statfs64,
#endif
#ifdef SYS_fstatfs
        SYS_fstatfs,
#endif
#ifdef SYS_fstatfs64
        SYS_fstatfs64,
#endif
#ifdef SYS_chmod
        SYS_chmod,
#endif
#ifdef SYS_fchmod
        SYS_fchmod,
#endif
#ifdef SYS_fchmodat
        SYS_fchmodat,
#endif
#ifdef SYS_umask
        SYS_umask,
#endif
#ifdef SYS_dup
        SYS_dup,
#endif
#ifdef SYS_dup2
        SYS_dup2,
#endif
#ifdef SYS_dup3
        SYS_dup3,
#endif
#ifdef SYS_pipe
        SYS_pipe,
#endif
#ifdef SYS_pipe2
        SYS_pipe2,
#endif
#ifdef SYS_ioctl
        SYS_ioctl,
#endif
#ifdef SYS_mmap
        SYS_mmap,
#endif
#ifdef SYS_mmap2
        SYS_mmap2,
#endif
#ifdef SYS_munmap
        SYS_munmap,
#endif
#ifdef SYS_mremap
        SYS_mremap,
#endif
#ifdef SYS_mprotect
        SYS_mprotect,
#endif
#ifdef SYS_madvise
        SYS_madvise,
#endif
#ifdef SYS_brk
        SYS_brk,
#endif
#ifdef SYS_futex
        SYS_futex,
#endif
#ifdef SYS_futex_time64
        SYS_futex_time64,
#endif
#ifdef SYS_futex_waitv
        SYS_futex_waitv,
#endif
#ifdef SYS_set_robust_list
        SYS_set_robust_list,
#endif
#ifdef SYS_rseq
        SYS_rseq,
#endif
#ifdef SYS_arch_prctl
        SYS_arch_prctl,
#endif
#ifdef SYS_set_tid_address
        SYS_set_tid_address,
#endif
#ifdef SYS_gettid
        SYS_gettid,
#endif
#ifdef SYS_getpid
        SYS_getpid,
#endif
#ifdef SYS_getppid
        SYS_getppid,
#endif
#ifdef SYS_getuid
        SYS_getuid,
#endif
#ifdef SYS_geteuid
        SYS_geteuid,
#endif
#ifdef SYS_getgid
        SYS_getgid,
#endif
#ifdef SYS_getegid
        SYS_getegid,
#endif
#ifdef SYS_getuid32
        SYS_getuid32,
#endif
#ifdef SYS_geteuid32
        SYS_geteuid32,
#endif
#ifdef SYS_getgid32
        SYS_getgid32,
#endif
#ifdef SYS_getegid32
        SYS_getegid32,
#endif
#ifdef SYS_clock_gettime
        SYS_clock_gettime,
#endif
#ifdef SYS_clock_gettime64
        SYS_clock_gettime64,
#endif
#ifdef SYS_gettimeofday
        SYS_gettimeofday,
#endif
#ifdef SYS_time
        SYS_time,
#endif
#ifdef SYS_nanosleep
        SYS_nanosleep,
#endif
#ifdef SYS_clock_nanosleep
        SYS_clock_nanosleep,
#endif
#ifdef SYS_clock_nanosleep_time64
        SYS_clock_nanosleep_time64,
#endif
#ifdef SYS_sched_yield
        SYS_sched_yield,
#endif
#ifdef SYS_sched_getaffinity
        SYS_sched_getaffinity,
#endif
#ifdef SYS_sched_setaffinity
        SYS_sched_setaffinity,
#endif
#ifdef SYS_sched_getparam
        SYS_sched_getparam,
#endif
#ifdef SYS_sched_getscheduler
        SYS_sched_getscheduler,
#endif
#ifdef SYS_sched_get_priority_min
        SYS_sched_get_priority_min,
#endif
#ifdef SYS_sched_get_priority_max
        SYS_sched_get_priority_max,
#endif
#ifdef SYS_prlimit64
        SYS_prlimit64,
#endif
#ifdef SYS_getrlimit
        SYS_getrlimit,
#endif
#ifdef SYS_ugetrlimit
        SYS_ugetrlimit,
#endif
#ifdef SYS_setrlimit
        SYS_setrlimit,
#endif
#ifdef SYS_prctl
        SYS_prctl,
#endif
#ifdef SYS_rt_sigaction
        SYS_rt_sigaction,
#endif
#ifdef SYS_rt_sigprocmask
        SYS_rt_sigprocmask,
#endif
#ifdef SYS_rt_sigreturn
        SYS_rt_sigreturn,
#endif
#ifdef SYS_sigaltstack
        SYS_sigaltstack,
#endif
#ifdef SYS_rt_sigpending
        SYS_rt_sigpending,
#endif
#ifdef SYS_rt_sigsuspend
        SYS_rt_sigsuspend,
#endif
#ifdef SYS_tgkill
        SYS_tgkill,
#endif
#ifdef SYS_exit
        SYS_exit,
#endif
#ifdef SYS_exit_group
        SYS_exit_group,
#endif
#ifdef SYS_poll
        SYS_poll,
#endif
#ifdef SYS_ppoll
        SYS_ppoll,
#endif
#ifdef SYS_ppoll_time64
        SYS_ppoll_time64,
#endif
#ifdef SYS_select
        SYS_select,
#endif
#ifdef SYS__newselect
        SYS__newselect,
#endif
#ifdef SYS_pselect6
        SYS_pselect6,
#endif
#ifdef SYS_pselect6_time64
        SYS_pselect6_time64,
#endif
#ifdef SYS_epoll_create
        SYS_epoll_create,
#endif
#ifdef SYS_epoll_create1
        SYS_epoll_create1,
#endif
#ifdef SYS_epoll_ctl
        SYS_epoll_ctl,
#endif
#ifdef SYS_epoll_wait
        SYS_epoll_wait,
#endif
#ifdef SYS_epoll_pwait
        SYS_epoll_pwait,
#endif
#ifdef SYS_epoll_pwait2
        SYS_epoll_pwait2,
#endif
#ifdef SYS_eventfd
        SYS_eventfd,
#endif
#ifdef SYS_eventfd2
        SYS_eventfd2,
#endif
#ifdef SYS_timerfd_create
        SYS_timerfd_create,
#endif
#ifdef SYS_timerfd_settime
        SYS_timerfd_settime,
#endif
#ifdef SYS_timerfd_settime64
        SYS_timerfd_settime64,
#endif
#ifdef SYS_timerfd_gettime
        SYS_timerfd_gettime,
#endif
#ifdef SYS_timerfd_gettime64
        SYS_timerfd_gettime64,
#endif
#ifdef SYS_getrandom
        SYS_getrandom,
#endif
#ifdef SYS_uname
        SYS_uname,
#endif
#ifdef SYS_sysinfo
        SYS_sysinfo,
#endif
#ifdef SYS_getrusage
        SYS_getrusage,
#endif
#ifdef SYS_memfd_create
        SYS_memfd_create,
#endif
#ifdef SYS_restart_syscall
        SYS_restart_syscall,
#endif
    };
    struct sock_filter code[2 * (sizeof(allowed)/sizeof(allowed[0])) + 32];
    size_t n = 0;
    // Reject a syscall ABI switch before examining syscall numbers.
    code[n++] = BPF_STMT(BPF_LD|BPF_W|BPF_ABS, offsetof(struct seccomp_data, arch));
    code[n++] = BPF_JUMP(BPF_JMP|BPF_JEQ|BPF_K, GAMEPKG_AUDIT_ARCH, 1, 0);
    code[n++] = BPF_STMT(BPF_RET|BPF_K, SECCOMP_RET_KILL_PROCESS);
    code[n++] = BPF_STMT(BPF_LD|BPF_W|BPF_ABS, offsetof(struct seccomp_data, nr));
#ifdef SYS_clone3
    // glibc must fall back to clone, whose thread flags can be inspected here.
    code[n++] = BPF_JUMP(BPF_JMP|BPF_JEQ|BPF_K, SYS_clone3, 0, 1);
    code[n++] = BPF_STMT(BPF_RET|BPF_K, SECCOMP_RET_ERRNO|ENOSYS);
#endif
#ifdef SYS_clone
    code[n++] = BPF_JUMP(BPF_JMP|BPF_JEQ|BPF_K, SYS_clone, 0, 5);
    code[n++] = BPF_STMT(BPF_LD|BPF_W|BPF_ABS, offsetof(struct seccomp_data, args[0]));
    // CLONE_VM | CLONE_SIGHAND | CLONE_THREAD: only threads in this process.
    code[n++] = BPF_STMT(BPF_ALU|BPF_AND|BPF_K, 0x00010900);
    code[n++] = BPF_JUMP(BPF_JMP|BPF_JEQ|BPF_K, 0x00010900, 0, 1);
    code[n++] = BPF_STMT(BPF_RET|BPF_K, SECCOMP_RET_ALLOW);
    code[n++] = BPF_STMT(BPF_RET|BPF_K, SECCOMP_RET_ERRNO|EPERM);
#endif
    for(size_t i=0; i<sizeof(allowed)/sizeof(allowed[0]); i++)
    {
        code[n++] = BPF_JUMP(BPF_JMP|BPF_JEQ|BPF_K, allowed[i], 0, 1);
        code[n++] = BPF_STMT(BPF_RET|BPF_K, SECCOMP_RET_ALLOW);
    }
    code[n++] = BPF_STMT(BPF_RET|BPF_K, SECCOMP_RET_ERRNO|EPERM);
    struct sock_fprog program = { static_cast<unsigned short>(n), code };
    if(prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 ||
       syscall(SYS_seccomp, SECCOMP_SET_MODE_FILTER, SECCOMP_FILTER_FLAG_TSYNC, &program) != 0) _exit(2);
}
#undef GAMEPKG_AUDIT_ARCH
