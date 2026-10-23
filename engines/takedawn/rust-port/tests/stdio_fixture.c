#include <stdint.h>
#include <stdio.h>
uintptr_t port_stdin(void) { return (uintptr_t)stdin; }
uintptr_t port_stdout(void) { return (uintptr_t)stdout; }
uintptr_t port_stderr(void) { return (uintptr_t)stderr; }
int64_t port_int64_min(void) { return INT64_MIN; }
int port_log(uint32_t tag, uint64_t count) {
    int n = fprintf(stderr, "native-stream:%08x:%llu\n", tag, (unsigned long long)count);
    return fflush(stderr) ? -1 : n;
}
