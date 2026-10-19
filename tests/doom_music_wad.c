/* Optional proof using a player's WAD read-only, never copied into a fixture. */
#define main generated_fixture_tests_main
#include "doom_music.c"
#undef main
static uint32_t le32(const unsigned char *bytes) {
    return (uint32_t)bytes[0] | (uint32_t)bytes[1] << 8 |
           (uint32_t)bytes[2] << 16 | (uint32_t)bytes[3] << 24;
}
int main(int argc, char **argv) {
    assert(argc == 2);
    FILE *file = fopen(argv[1], "rb"); assert(file);
    assert(!fseek(file, 0, SEEK_END)); long size = ftell(file);
    assert(size >= 12 && size <= 64 * 1024 * 1024);
    rewind(file); unsigned char *wad = malloc((size_t)size); assert(wad);
    assert(fread(wad, 1, (size_t)size, file) == (size_t)size); fclose(file);
    assert(!memcmp(wad, "IWAD", 4));
    uint32_t count = le32(wad + 4), offset = le32(wad + 8);
    assert(count && count <= 65536 && offset <= (uint32_t)size);
    assert(count <= ((uint32_t)size - offset) / 16);
    int have_instruments = 0;
    for (uint32_t i = 0; i < count; i++) {
        const unsigned char *entry = wad + offset + i * 16;
        uint32_t start = le32(entry), length = le32(entry + 4);
        assert(start <= (uint32_t)size && length <= (uint32_t)size - start);
        if (!memcmp(entry + 8, "GENMIDI", 7)) {
            assert(length >= sizeof(instruments));
            memcpy(instruments, wad + start, sizeof(instruments)); have_instruments = 1;
        }
    }
    assert(have_instruments && DG_music_module.Init());
    DG_music_module.SetMusicVolume(127);
    unsigned songs = 0;
    for (uint32_t i = 0; i < count; i++) {
        const unsigned char *entry = wad + offset + i * 16;
        uint32_t start = le32(entry), length = le32(entry + 4);
        if (length < 4 || memcmp(entry + 8, "D_", 2) ||
            (memcmp(wad + start, "MUS\x1a", 4) && memcmp(wad + start, "MThd", 4))) continue;
        void *song = DG_music_module.RegisterSong(wad + start, length);
        assert(song); DG_music_module.PlaySong(song, true);
        assert(render(180) > 0);
        DG_music_module.StopSong(); DG_music_module.UnRegisterSong(song);
        printf("Verified music %.8s\n", entry + 8); songs++;
    }
    assert(songs > 20);
    DG_music_module.Shutdown(); free(wad);
    printf("Verified %u original music lumps, RAM-only, 3 seconds per song.\n", songs);
    return 0;
}
