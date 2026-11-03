"""Qualify executed paths without an instruction-name allowlist."""
from pathlib import Path
import subprocess
import sys
import tempfile

PROJECT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(PROJECT / "tools"))
from check_takedawn_paths import require_supported_paths

with tempfile.TemporaryDirectory(prefix="gamepkg-path-gate-") as directory:
    root = Path(directory)

    def check(parts, fails):
        paths = []
        for index, content in enumerate(parts):
            path = root / f"{index}.log"
            path.write_bytes(content)
            paths.append(path)
        try:
            require_supported_paths(paths)
        except ValueError as error:
            assert fails and fails in str(error), str(error)
        else:
            assert not fails, "an unsupported executed path was qualified"

    check([], "no runtime logs")
    check([b"[GPU] flip 120\r\n", b"\xff\xfe"], None)
    check([b"RECOMP_UNIMPL_TRAP=1\nrecomp_stubs_unresolved.rs\n"], None)
    for instruction in ["stmxcsr", "ldmxcsr", "wbinvd", "cli", "sti", "fyl2x", "paddq", "future-op"]:
        marker = f"[UNIMPL] untranslated instruction REACHED: `{instruction}` at 0x00353DB5\r\n".encode()
        check([marker], "untranslated instruction")
        check([b"clean\n", b"x" * (65536 - 13) + marker], "untranslated instruction")
        check([marker, b"later clean restart\n"], "untranslated instruction")
    stub = b"[STUB] unresolved 0xE9DAEA40 reached, return 0x00000000 esp 0x12345678\n"
    check([stub], "unresolved upstream")
    check([b"x" * (65536 - 5) + stub], "unresolved upstream")
    result = subprocess.run([sys.executable, PROJECT / "tools/check_takedawn_paths.py", root / "missing.log"], capture_output=True, text=True)
    assert result.returncode != 0 and "FAIL:" in result.stderr

print("Path gate passed: all untranslated names, reached stubs, restart history, split chunks and missing logs.")
