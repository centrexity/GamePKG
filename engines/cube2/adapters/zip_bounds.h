#ifndef GAMEPKG_ZIP_BOUNDS_H
#define GAMEPKG_ZIP_BOUNDS_H
static inline unsigned gamepkg_zip_u16(const unsigned char *p)
{
    return (unsigned)p[0] | ((unsigned)p[1] << 8);
}
static inline unsigned gamepkg_zip_u32(const unsigned char *p)
{
    return gamepkg_zip_u16(p) | (gamepkg_zip_u16(p + 2) << 16);
}
// Only inspect candidates with a complete end-directory header in this buffer.
static inline int gamepkg_zip_find_end(const unsigned char *bytes, unsigned length, unsigned next, unsigned long long tail = 0)
{
    if(next > length) return -1;
    while(next)
    {
        unsigned at = --next;
        if(length - at < 22u || gamepkg_zip_u32(bytes + at) != 0x06054b50u) continue;
        unsigned remaining = length - at - 22u;
        if(tail <= 65535u && remaining <= 65535u - tail &&
            gamepkg_zip_u16(bytes + at + 20) == remaining + tail) return (int)at;
    }
    return -1;
}
// Validate the physical directory extent before allocating its declared size.
static inline bool gamepkg_zip_directory_valid(long long total, long long offset,
    unsigned long long size, int entries)
{
    if(total < 0 || offset < 0 || entries <= 0) return false;
    unsigned long long end = (unsigned long long)total, start = (unsigned long long)offset;
    return start <= end && size <= end - start && size / 46u >= (unsigned)entries;
}
// zlib accepts an unsigned int output count. Clamp before narrowing and never
// deliver output beyond the central entry's declared uncompressed length.
static inline unsigned gamepkg_zip_read_size(unsigned long long declared,
    unsigned long long produced, unsigned long long requested)
{
    if(produced >= declared) return 0;
    unsigned long long remaining = declared - produced;
    if(requested > remaining) requested = remaining;
    if(requested > 0xffffffffull) requested = 0xffffffffull;
    return (unsigned)requested;
}
// The original stream uses 32-bit offsets. Validate before any addition,
// including the local name/extra fields and the complete compressed payload.
static inline bool gamepkg_zip_payload_valid(long long total,
    unsigned long long header, unsigned long long name,
    unsigned long long extra, unsigned long long payload)
{
    if(total < 0 || header > 0xffffffffull) return false;
    unsigned long long available = (unsigned long long)total;
    if(available > 0xffffffffull) available = 0xffffffffull;
    if(header > available) return false;
    available -= header;
    if(available < 30u) return false;
    available -= 30u;
    if(name > available) return false;
    available -= name;
    if(extra > available) return false;
    return payload <= available - extra;
}
// Header fields are unsigned 16-bit on disk; subtraction keeps the guard safe
// even when called with wider values by the boundary regression.
static inline bool gamepkg_zip_entry_valid(unsigned long long remaining,
    unsigned long long name, unsigned long long extra, unsigned long long comment)
{
    if(name > remaining) return false;
    remaining -= name;
    if(extra > remaining) return false;
    return comment <= remaining - extra;
}
#endif
