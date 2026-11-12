#include <cassert>
#include <climits>
#include "../engines/cube2/adapters/zip_bounds.h"
int main()
{
    unsigned char bytes[1050] = {};
    for(unsigned offset = 0; offset < 1024; ++offset)
    {
        for(unsigned i = 0; i < sizeof(bytes); ++i) bytes[i] = 0;
        bytes[offset] = 0x50; bytes[offset+1] = 0x4b;
        bytes[offset+2] = 0x05; bytes[offset+3] = 0x06;
        assert(gamepkg_zip_u16(bytes + offset) == 0x4b50u);
        assert(gamepkg_zip_u32(bytes + offset) == 0x06054b50u);
        assert(gamepkg_zip_find_end(bytes, offset + 22, offset + 1) == (int)offset);
        assert(gamepkg_zip_find_end(bytes, offset + 21, offset + 1) == -1);
    }
    assert(gamepkg_zip_find_end(bytes, 0, 0) == -1);
    assert(gamepkg_zip_find_end(bytes, 1, 2) == -1);
    unsigned char decoy[62] = {};
    decoy[0] = decoy[22] = 0x50; decoy[1] = decoy[23] = 0x4b;
    decoy[2] = decoy[24] = 0x05; decoy[3] = decoy[25] = 0x06;
    decoy[20] = 40; // Real record's complete comment includes the decoy.
    assert(gamepkg_zip_find_end(decoy, sizeof(decoy), sizeof(decoy)) == 0);
    unsigned char split[22] = {0x50, 0x4b, 0x05, 0x06};
    split[20] = 0xff; split[21] = 0xff;
    assert(gamepkg_zip_find_end(split, sizeof(split), sizeof(split), 65535) == 0);
    assert(gamepkg_zip_find_end(split, sizeof(split), sizeof(split), 65534) == -1);
    assert(gamepkg_zip_find_end(split, sizeof(split), sizeof(split), ULLONG_MAX) == -1);
    unsigned char tail[1024] = {};
    tail[1023] = 0x50;
    assert(gamepkg_zip_find_end(tail, sizeof(tail), sizeof(tail)) == -1);
    assert(gamepkg_zip_read_size(10, 0, 20) == 10);
    assert(gamepkg_zip_read_size(10, 9, 20) == 1);
    assert(gamepkg_zip_read_size(10, 10, 20) == 0);
    assert(gamepkg_zip_read_size(10, 11, 20) == 0);
    assert(gamepkg_zip_read_size(ULLONG_MAX, 0, ULLONG_MAX) == UINT_MAX);
    assert(gamepkg_zip_read_size(ULLONG_MAX, ULLONG_MAX - 1, ULLONG_MAX) == 1);
    assert(gamepkg_zip_read_size(10, 0, 0) == 0);
    assert(gamepkg_zip_payload_valid(100, 10, 5, 5, 50));
    assert(!gamepkg_zip_payload_valid(99, 10, 5, 5, 50));
    assert(gamepkg_zip_payload_valid(30, 0, 0, 0, 0));
    assert(!gamepkg_zip_payload_valid(29, 0, 0, 0, 0));
    assert(!gamepkg_zip_payload_valid(-1, 0, 0, 0, 0));
    assert(!gamepkg_zip_payload_valid(100, 101, 0, 0, 0));
    assert(!gamepkg_zip_payload_valid(LLONG_MAX, ULLONG_MAX, 0, 0, 0));
    assert(!gamepkg_zip_payload_valid(LLONG_MAX, 0, ULLONG_MAX, 0, 0));
    assert(!gamepkg_zip_payload_valid(LLONG_MAX, 0, 0, ULLONG_MAX, 0));
    assert(!gamepkg_zip_payload_valid(LLONG_MAX, 0, 0, 0, ULLONG_MAX));
    assert(gamepkg_zip_payload_valid(LLONG_MAX, UINT_MAX - 30ull, 0, 0, 0));
    assert(!gamepkg_zip_payload_valid(LLONG_MAX, UINT_MAX - 29ull, 0, 0, 0));
    assert(gamepkg_zip_payload_valid(LLONG_MAX, 0, 0, 0, UINT_MAX - 30ull));
    assert(!gamepkg_zip_payload_valid(LLONG_MAX, 0, 0, 0, UINT_MAX - 29ull));
    assert(gamepkg_zip_entry_valid(9, 3, 4, 2));
    assert(gamepkg_zip_entry_valid(0, 0, 0, 0));
    assert(!gamepkg_zip_entry_valid(8, 3, 4, 2));
    assert(!gamepkg_zip_entry_valid(8, 9, 0, 0));
    assert(!gamepkg_zip_entry_valid(8, 0, 9, 0));
    assert(!gamepkg_zip_entry_valid(8, 0, 0, 9));
    assert(!gamepkg_zip_entry_valid(ULLONG_MAX, ULLONG_MAX, 1, 0));
    assert(!gamepkg_zip_entry_valid(ULLONG_MAX, 0, ULLONG_MAX, 1));
    assert(gamepkg_zip_entry_valid(ULLONG_MAX, ULLONG_MAX, 0, 0));
    assert(gamepkg_zip_directory_valid(100, 54, 46, 1));
    assert(gamepkg_zip_directory_valid(100, 0, 92, 2));
    assert(!gamepkg_zip_directory_valid(100, 55, 46, 1));
    assert(!gamepkg_zip_directory_valid(100, 101, 0, 1));
    assert(!gamepkg_zip_directory_valid(-1, 0, 46, 1));
    assert(!gamepkg_zip_directory_valid(100, -1, 46, 1));
    assert(!gamepkg_zip_directory_valid(100, 0, 46, 0));
    assert(!gamepkg_zip_directory_valid(100, 0, 46, -1));
    assert(!gamepkg_zip_directory_valid(100, 0, 91, 2));
    assert(!gamepkg_zip_directory_valid(100, 0, UINT_MAX, 1));
    assert(!gamepkg_zip_directory_valid(LLONG_MAX, LLONG_MAX, ULLONG_MAX, 1));
    assert(gamepkg_zip_directory_valid(LLONG_MAX, LLONG_MAX - 46, 46, 1));
    assert(!gamepkg_zip_directory_valid(LLONG_MAX, LLONG_MAX - 45, 46, 1));
    assert(!gamepkg_zip_directory_valid(LLONG_MAX, 0, 46, INT_MAX));
    assert(gamepkg_zip_directory_valid(LLONG_MAX, 0, 46ull * INT_MAX, INT_MAX));
}
