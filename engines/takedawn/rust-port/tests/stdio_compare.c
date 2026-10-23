#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdarg.h>
void *gamepkg_native_stdin(void) { return stdin; }
void *gamepkg_native_stdout(void) { return stdout; }
void *gamepkg_native_stderr(void) { return stderr; }
int gamepkg_native_fflush(void *stream) { return fflush((FILE *)stream); }
int gamepkg_native_fprintf(void *stream, const char *format, ...) {
    va_list args;
    va_start(args, format);
    int result = vfprintf((FILE *)stream, format, args);
    va_end(args);
    return result;
}
uintptr_t port_stdin(void), port_stdout(void), port_stderr(void);
uintptr_t ref_port_stdin(void), ref_port_stdout(void), ref_port_stderr(void);
int port_log(uint32_t, uint64_t), ref_port_log(uint32_t, uint64_t);
int64_t port_int64_min(void), ref_port_int64_min(void);
int main(void) {
    assert(port_int64_min() == INT64_MIN && port_int64_min() == ref_port_int64_min());
    assert(port_stdin() == ref_port_stdin() && port_stdin() == (uintptr_t)stdin);
    assert(port_stdout() == ref_port_stdout() && port_stdout() == (uintptr_t)stdout);
    assert(port_stderr() == ref_port_stderr() && port_stderr() == (uintptr_t)stderr);
    const uint64_t values[] = {0, 1, UINT32_MAX, UINT64_C(0x100000000),
                              UINT64_C(0x7FFFFFFFFFFFFFFF), UINT64_MAX};
    for (unsigned i = 0; i < sizeof values / sizeof *values; i++) {
        int reference = ref_port_log(UINT32_C(0x87654321) + i, values[i]);
        int converted = port_log(UINT32_C(0x87654321) + i, values[i]);
        assert(reference > 0 && converted == reference);
    }
    puts("PASS: native C/Rust stream identity, fixed-width minimum, 32/64-bit variadic values and flush results");
    return 0;
}
