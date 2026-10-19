// Test the actual host-owned guard, not a duplicate model of its policy.
#include "../engines/cube2/adapters/syscall_guard.h"
#include <sys/socket.h>
#include <pthread.h>
#include <stdio.h>
static void *thread(void *arg) {
    if(socket(AF_INET, SOCK_DGRAM, 0) != -1 || errno != EPERM) return NULL;
    return arg;
}
int main()
{
    gamepkg_lockdown();
    if(prctl(PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) != 1) return 10;
    if(prctl(PR_GET_SECCOMP, 0, 0, 0, 0) != SECCOMP_MODE_FILTER) return 11;
    if(socket(AF_INET, SOCK_DGRAM, 0) != -1 || errno != EPERM) return 12;
    if(socket(AF_INET6, SOCK_STREAM, 0) != -1 || errno != EPERM) return 13;
    if(socket(AF_UNIX, SOCK_STREAM, 0) != -1 || errno != EPERM) return 14;
#ifdef SYS_fork
    if(syscall(SYS_fork) != -1 || errno != EPERM) return 15;
#endif
#ifdef SYS_vfork
    if(syscall(SYS_vfork) != -1 || errno != EPERM) return 16;
#endif
#ifdef SYS_clone
    if(syscall(SYS_clone, 17, 0, 0, 0, 0) != -1 || errno != EPERM) return 17;
#endif
#ifdef SYS_clone3
    if(syscall(SYS_clone3, 0, 0) != -1 || errno != ENOSYS) return 18;
#endif
    char *args[] = {const_cast<char *>("true"), NULL};
    if(execve("/usr/bin/true", args, NULL) != -1 || errno != EPERM) return 19;
#ifdef SYS_execveat
    if(syscall(SYS_execveat, -100, "/usr/bin/true", args, NULL, 0) != -1 || errno != EPERM) return 20;
#endif
    if(syscall(999999) != -1 || errno != EPERM) return 21;
    pthread_t worker;
    void *result = NULL;
    int argument = 123;
    if(pthread_create(&worker, NULL, thread, &argument) != 0) return 22;
    if(pthread_join(worker, &result) != 0 || result != &argument) return 23;
    puts("Cube 2 guard: sockets, fork/vfork/process clone, exec and unknown syscalls denied; pthreads allowed.");
}
