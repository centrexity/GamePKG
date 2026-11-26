"""Read normal/delay PE imports without executing a binary or loading a DLL.

Layout: https://learn.microsoft.com/en-us/windows/win32/debug/pe-format
"""
import re
import struct
from pathlib import Path


def imports(path):
    if Path(path).stat().st_size > 256 * 1024 * 1024:
        raise ValueError("PE app exceeds the release inspection size limit")
    data = Path(path).read_bytes()

    def unpack(fmt, offset):
        size = struct.calcsize(fmt)
        if offset < 0 or offset + size > len(data):
            raise ValueError("truncated PE structure")
        return struct.unpack_from(fmt, data, offset)

    if data[:2] != b"MZ":
        raise ValueError("expected a Windows executable")
    nt, = unpack("<I", 60)
    if data[nt:nt+4] != b"PE\0\0":
        raise ValueError("invalid PE signature")
    sections, = unpack("<H", nt + 6)
    optional_size, = unpack("<H", nt + 20)
    optional = nt + 24
    magic, = unpack("<H", optional)
    if magic == 0x10b:
        directory_offset, count_offset = 96, 92
        image_base, = unpack("<I", optional + 28)
    elif magic == 0x20b:
        directory_offset, count_offset = 112, 108
        image_base, = unpack("<Q", optional + 24)
    else:
        raise ValueError("unsupported PE optional header")
    count, = unpack("<I", optional + count_offset)
    if count > 16 or optional_size < directory_offset + count * 8:
        raise ValueError("invalid PE data directories")
    headers_size, = unpack("<I", optional + 60)
    ranges = []
    for index in range(sections):
        section = optional + optional_size + index * 40
        va, raw_size, raw_offset = unpack("<III", section + 12)
        if raw_offset + raw_size > len(data):
            raise ValueError("truncated PE section")
        ranges.append((va, raw_size, raw_offset))

    def offset(rva, size):
        if 0 <= rva < headers_size and rva + size <= min(headers_size, len(data)):
            return rva
        matches = [raw + rva - va for va, length, raw in ranges
                   if va <= rva and rva + size <= va + length]
        if len(matches) != 1:
            raise ValueError("unbacked or ambiguous PE import address")
        return matches[0]

    def dll_name(rva):
        value = bytearray()
        for index in range(512):
            byte = data[offset(rva + index, 1)]
            if not byte:
                name = value.decode("ascii")
                if not re.fullmatch(r"[A-Za-z0-9_.+-]+\.dll", name, re.IGNORECASE):
                    raise ValueError("invalid PE DLL name")
                return name.lower()
            value.append(byte)
        raise ValueError("unterminated PE DLL name")

    found = set()
    for entry, descriptor_size in [(1, 20), (13, 32)]:
        if entry >= count:
            continue
        rva, size = unpack("<II", optional + directory_offset + entry * 8)
        if not rva and not size:
            continue
        if not rva or size < descriptor_size or size > 1024 * 1024:
            raise ValueError("invalid PE import directory")
        for index in range(min(size // descriptor_size, 4096)):
            words = unpack("<" + "I" * (descriptor_size // 4), offset(rva + index * descriptor_size, descriptor_size))
            if not any(words):
                break
            name = words[3] if entry == 1 else words[1]
            if entry == 13 and not (words[0] & 1):
                name -= image_base
            found.add(dll_name(name))
        else:
            raise ValueError("unterminated PE import directory")
    return sorted(found)


def require_standalone_crt(path):
    names = imports(path)
    external = [name for name in names if re.match(
        r"(?:vcruntime|msvcp\d|msvcr\d|libgcc_s_|libstdc\+\+|libwinpthread)", name
    )]
    if external:
        raise ValueError("Windows app requires external compiler runtime: " + ", ".join(external))
    return names


def require_application_icon(path):
    """Require a valid Win32 RT_GROUP_ICON pointing at real RT_ICON data."""
    path = Path(path)
    if path.stat().st_size > 256 * 1024 * 1024:
        raise ValueError("PE app exceeds the release inspection size limit")
    data = path.read_bytes()

    def unpack(fmt, offset):
        size = struct.calcsize(fmt)
        if offset < 0 or offset + size > len(data):
            raise ValueError("truncated PE structure")
        return struct.unpack_from(fmt, data, offset)

    if data[:2] != b"MZ":
        raise ValueError("expected a Windows executable")
    nt, = unpack("<I", 60)
    if data[nt:nt+4] != b"PE\0\0":
        raise ValueError("invalid PE signature")
    sections, = unpack("<H", nt + 6)
    optional_size, = unpack("<H", nt + 20)
    optional = nt + 24
    magic, = unpack("<H", optional)
    if magic == 0x10b:
        directory_offset, count_offset = 96, 92
    elif magic == 0x20b:
        directory_offset, count_offset = 112, 108
    else:
        raise ValueError("unsupported PE optional header")
    count, = unpack("<I", optional + count_offset)
    if count < 3 or optional_size < directory_offset + count * 8:
        raise ValueError("PE has no resource directory")
    resource_rva, resource_size = unpack(
        "<II", optional + directory_offset + 2 * 8)
    if not resource_rva or resource_size < 16:
        raise ValueError("PE has no application resources")
    headers_size, = unpack("<I", optional + 60)
    ranges = []
    for index in range(sections):
        section = optional + optional_size + index * 40
        va, raw_size, raw_offset = unpack("<III", section + 12)
        if raw_offset + raw_size > len(data):
            raise ValueError("truncated PE section")
        ranges.append((va, raw_size, raw_offset))

    def offset(rva, size):
        if 0 <= rva < headers_size and rva + size <= min(headers_size, len(data)):
            return rva
        matches = [raw + rva - va for va, length, raw in ranges
                   if va <= rva and rva + size <= va + length]
        if len(matches) != 1:
            raise ValueError("unbacked or ambiguous PE resource address")
        return matches[0]

    def directory_entries(directory):
        named, ids = unpack("<HH", directory + 12)
        if named + ids > 4096:
            raise ValueError("oversized PE resource directory")
        for index in range(named + ids):
            name, target = unpack("<II", directory + 16 + index * 8)
            yield name, target

    def leaves(directory, depth=0):
        if depth > 2:
            raise ValueError("deep PE resource directory")
        for name, target in directory_entries(directory):
            if target & 0x80000000:
                child = resource + (target & 0x7fffffff)
                yield from leaves(child, depth + 1)
            else:
                yield name, target

    resource = offset(resource_rva, 16)
    icons = {3: [], 14: []}
    for type_id in icons:
        for name, target in directory_entries(resource):
            if name != type_id:
                continue
            entries = ([target] if not target & 0x80000000 else
                       [entry for _, entry in leaves(resource + (target & 0x7fffffff))])
            for entry in entries:
                if entry & 0x80000000:
                    continue
                data_rva, size = unpack("<II", resource + entry)
                if size:
                    icons[type_id].append(
                        (offset(data_rva, min(size, 64 * 1024 * 1024)), size))
    if not icons[3] or not icons[14]:
        raise ValueError("PE lacks an RT_GROUP_ICON/RT_ICON pair")
    group, size = icons[14][0]
    if size < 6:
        raise ValueError("truncated RT_GROUP_ICON")
    reserved, kind, images = unpack("<HHH", group)
    if reserved or kind != 1 or not 1 <= images <= 256 or size < 6 + images * 14:
        raise ValueError("invalid RT_GROUP_ICON")
    if len(icons[3]) < images:
        raise ValueError("RT_GROUP_ICON lists more images than RT_ICON provides")
    dimensions = []
    for index in range(images):
        width, height, _, _, planes, bits, _, identifier = unpack(
            "<BBBBHHIH", group + 6 + index * 14)
        if planes != 1 or bits not in (1, 4, 8, 16, 24, 32):
            raise ValueError("invalid RT_GROUP_ICON image description")
        dimensions.append((width or 256, height or 256, identifier))
    if not any(width >= 32 and height >= 32 for width, height, _ in dimensions):
        raise ValueError("RT_GROUP_ICON has no 32px-or-larger image")
    return dimensions
