"""Maintainer-only insertion of the reviewed Windows worker placement policy."""


def patch_kernel_bridge(source: str, implementation: str) -> str:
    include = '#include "kernel.h"\n'
    role = 'void xbox_log_thread_role(const char *role, uint32_t routine)\n{\n'
    if 'GAMEPKG_NATIVE_WINDOWS_CPU_H' in source:
        raise ValueError('Windows CPU policy is already present')
    if source.count(include) != 1 or source.count(role) != 1:
        raise ValueError('Pinned kernel bridge insertion points differ')
    if '#define GAMEPKG_NATIVE_WINDOWS_CPU_H' not in implementation:
        raise ValueError('Missing reviewed Windows CPU implementation')
    source = source.replace(include, include + '#if defined(_WIN32)\n'
                            + implementation + '\n#endif\n', 1)
    return source.replace(role, role + '#if defined(_WIN32)\n'
                          '    if (role && !strcmp(role, "nv2a-ack"))\n'
                          '        gamepkg_windows_pin_executor();\n'
                          '#endif\n', 1)
