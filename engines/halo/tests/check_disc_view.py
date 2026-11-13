#!/usr/bin/env python3
"""Maintainer-only native disc-reader checks; no game assets or player Python dependency."""
import argparse
import os
from pathlib import Path
import struct
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--source", required=True, type=Path, help="pinned engine checkout with reviewed patches applied")
parser.add_argument("--cflag", action="append", default=[], help="additional maintainer compiler argument")
parser.add_argument("--i686-sysroot", type=Path, help="run actual 32-bit code using this existing C sysroot")
args = parser.parse_args()
ROOT = Path(__file__).resolve().parent
SOURCE = args.source.resolve()

def entry(name, sector, size, directory=False, right=0):
    name = name.encode()
    return struct.pack("<HHIIBB", 0, right, sector, size, 16 if directory else 0, len(name)) + name

def image(path, fault=None, sparse=False):
    sector = 1_200_000 if sparse else 44
    root = entry("maps", 42, 64, True)
    table = bytearray(entry("ui.map", sector, 10).ljust(64, b"\xff"))
    if fault == "traversal": table[:21] = entry("../evil", sector, 10)
    if fault == "extent": struct.pack_into("<I", table, 8, 0xffffffff)
    if fault == "cycle": struct.pack_into("<H", table, 2, 6); table[24:44] = entry("xx.map", sector, 10, right=6)
    if fault == "duplicate": struct.pack_into("<H", table, 2, 6); table[24:44] = entry("UI.MAP", sector, 10)
    if fault == "link": struct.pack_into("<H", table, 2, 65534)
    if fault == "missing-ui": table[:20] = entry("xx.map", sector, 10)
    descriptor = bytearray(2048)
    descriptor[:20] = descriptor[0x7ec:0x800] = b"MICROSOFT*XBOX*MEDIA"
    struct.pack_into("<II", descriptor, 0x14, 40, 64)
    if fault == "magic": descriptor[0] = 0
    with path.open("wb") as f:
        f.seek(0x10000); f.write(descriptor)
        f.seek(40 * 2048); f.write(root.ljust(64, b"\xff"))
        f.seek(42 * 2048); f.write(table)
        f.seek(sector * 2048); f.write(b"0123456789")
    if fault == "truncated": path.write_bytes(path.read_bytes()[:2048])

with tempfile.TemporaryDirectory(prefix="gamepkg-disc-view-") as temp:
    temp = Path(temp)
    (temp / "fixture.c").write_bytes((ROOT / "disc_view.c").read_bytes())
    (temp / "platform-stub.h").write_text("#define __HALO_LINUX_PLATFORM_H\nvoid platform_log(const char *, ...);\n")
    flags = ["-g", "-O1", "-D_FILE_OFFSET_BITS=64", "-include", str(temp / "platform-stub.h"), "-I", str(SOURCE / "port/linux/src"), "-pthread"]
    flags.extend(args.cflag)
    sources = [str(temp / "fixture.c"), str(SOURCE / "port/linux/src/xiso.c"), str(SOURCE / "port/linux/src/posix_files.c")]
    binary = temp / "disc-fixture"
    if args.i686_sysroot:
        flags.extend(["-m32", "--sysroot=" + str(args.i686_sysroot.resolve())])
    subprocess.run(["clang", *flags, *(["-fsanitize=address,undefined"] if not args.i686_sysroot else []), *sources, "-o", str(binary)], check=True)
    env = dict(os.environ, ASAN_OPTIONS="detect_leaks=1:abort_on_error=1", UBSAN_OPTIONS="halt_on_error=1")
    cases = [None, "traversal", "extent", "cycle", "duplicate", "link", "missing-ui", "magic", "truncated"]
    for fault in cases:
        path = temp / "fixture.iso"
        image(path, fault)
        subprocess.run([str(binary), str(path), "bad" if fault else "good"], env=env, check=True)
        print("PASS", fault or "valid", flush=True)
    image(path, sparse=True)
    subprocess.run([str(binary), str(path), "good"], env=env, check=True)
    print("PASS sparse reads beyond 2 GiB; 8 concurrent readers; no extraction", flush=True)
