"""Reject requested-but-unavailable graphics and mixed restart outcomes."""
from pathlib import Path
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
from check_takedawn_renderer import require_vulkan

READY = b"[vulkan] RECOMP_PB_BACKEND=vulkan: executor draws through Vulkan\r\n"
FAILED = (b"[vulkan] device up: NVIDIA GeForce RTX 3090\r\n"
          b"[vulkan] ring buffer 2 failed\r\n"
          b"[vulkan] RECOMP_PB_BACKEND=vulkan: no device, staying on the CPU rasteriser\r\n")

with tempfile.TemporaryDirectory(prefix="gamepkg-renderer-gate-") as temporary:
    root = Path(temporary)

    def check(parts, succeeds):
        paths = []
        for index, content in enumerate(parts):
            path = root / f"{index}.log"
            path.write_bytes(content)
            paths.append(path)
        try:
            require_vulkan(paths)
        except ValueError:
            assert not succeeds, "working Vulkan rendering was rejected"
        else:
            assert succeeds, "unconfirmed or fallback rendering was qualified"

    check([READY, b""], True)
    check([b"[vulkan] device up: NVIDIA GeForce RTX 3090\r\n"], False)
    check([FAILED], False)
    check([READY, FAILED], False)
    check([FAILED, READY], False)
    check([b"", b"\xff\xfe"], False)
    check([b"x" * (65536 - 17) + READY], True)
    fallback_line = FAILED.splitlines(keepends=True)[-1]
    check([READY + b"x" * (65536 - len(READY) - 17) + fallback_line], False)

print("Renderer gate passed: actual executor, device-only failure, CPU fallback, restart outcomes and split log chunks.")
