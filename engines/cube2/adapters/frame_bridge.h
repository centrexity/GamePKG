// GamePKG adaptation, not part of upstream Cube 2. Zlib license; see ../upstream/readme_source.txt.
// Development bridge: software OpenGL remains private to the engine process.
// No window-system connection or GPU handle crosses this byte protocol.
#include <unistd.h>
#include <stdint.h>
#include <errno.h>
#include "syscall_guard.h"
#include <SDL_mixer.h>
#include <dirent.h>
#include <fcntl.h>
#include <sys/stat.h>
#include <sys/statvfs.h>
#include <string>
#include <vector>
#include <algorithm>

extern void quit();
static int gamepkg_output = -1;
static void gamepkg_prepare();
static bool gamepkg_running = false;
static bool gamepkg_closing = false;
static uint32_t gamepkg_previous = 0;
static bool gamepkg_keys[232] = {};
static bool gamepkg_mouse[6] = {};
static bool gamepkg_transfer(int fd, void *data, size_t length, bool output)
{
    unsigned char *p = static_cast<unsigned char *>(data);
    while(length)
    {
        ssize_t n = output ? write(fd, p, length) : read(fd, p, length);
        if(n < 0 && errno == EINTR) continue;
        if(n <= 0) return false;
        p += n; length -= n;
    }
    return true;
}
static uint32_t gamepkg_le32(const unsigned char *p)
{
    return uint32_t(p[0]) | uint32_t(p[1])<<8 | uint32_t(p[2])<<16 | uint32_t(p[3])<<24;
}
static void gamepkg_put32(unsigned char *p, uint32_t n)
{
    for(int i=0;i<4;i++) p[i] = (n>>(8*i))&255;
}
#include "persistence.h"
static const char *gamepkg_save_names[] = { "/save/config.cfg", "/save/init.cfg", "/save/servers.cfg" };
static const size_t gamepkg_save_limit = 1048576-16;
static void gamepkg_prepare()
{
    gamepkg_lockdown();
    // Keep only stdin, diagnostics and one frame channel. In particular no
    // directory capability used by the namespace launcher reaches game code.
    // Descriptor 3 is the host networking broker, never an external socket.
    if(dup2(1, 4) != 4 || dup2(2, 1) < 0 || syscall(SYS_close_range, 5u, ~0u, 0u) != 0) exit(2);
    gamepkg_output = 4;
    unsigned char header[16];
    if(!gamepkg_transfer(0, header, sizeof(header), false)) exit(2);
    if(!memcmp(header, "GPB2", 4)) { gamepkg_restore_tree(header); return; }
    if(memcmp(header, "GPB1", 4)) exit(2);
    size_t total = 0;
    for(unsigned i=0;i<3;i++)
    {
        uint32_t length = gamepkg_le32(header+4+i*4);
        if(length > gamepkg_save_limit-total) exit(2);
        total += length;
    }
    for(unsigned i=0;i<3;i++)
    {
        uint32_t length = gamepkg_le32(header+4+i*4);
        if(!length) continue;
        vector<unsigned char> bytes; bytes.reserve(length); bytes.advance(length);
        if(!gamepkg_transfer(0, bytes.getbuf(), length, false)) exit(2);
        FILE *file = fopen(gamepkg_save_names[i], "wb");
        if(!file) exit(2);
        bool okay = fwrite(bytes.getbuf(), 1, length, file) == length;
        if(fclose(file) != 0 || !okay) exit(2);
    }
}
static void gamepkg_snapshot()
{
    struct statvfs space;
    if(statvfs("/save", &space) || !space.f_frsize || uint64_t(space.f_bavail)*space.f_frsize < 1048576) gamepkg_save_failure();
    extern void writeinitcfg();
    writecfg(); writeinitcfg(); writeservercfg();
    gamepkg_tree_snapshot(gamepkg_output);
}
static void gamepkg_quit()
{
    // A game-menu Quit is an orderly protocol event, never inferred from EOF.
    // Host-requested close already collected its snapshot and expects no output.
    if(!gamepkg_running || gamepkg_closing) return;
    gamepkg_closing = true;
    unsigned char magic[4] = { 'G', 'P', 'Q', '1' };
    if(!gamepkg_transfer(gamepkg_output, magic, sizeof(magic), true)) return;
    gamepkg_snapshot();
}
static void gamepkg_input()
{
    // Fixed 16-byte input: GPK1, held-key mask, signed mouse dx and dy.
    unsigned char packet[16];
    unsigned events = 0;
    for(;;)
    {
        if(!gamepkg_transfer(0, packet, sizeof(packet), false)) { gamepkg_closing = true; quit(); exit(0); }
        if(!memcmp(packet, "GPS1", 4)) { gamepkg_snapshot(); continue; }
        if(!memcmp(packet, "GPE1", 4)) { gamepkg_closing = true; quit(); exit(0); }
        if(!memcmp(packet, "GPC1", 4))
        {
            if(++events > 64) exit(2);
            uint32_t button=gamepkg_le32(packet+4), pressed=gamepkg_le32(packet+8), mods=gamepkg_le32(packet+12);
            if((button!=2 && button!=4 && button!=5) || pressed>1 || (mods & ~uint32_t(0x0fc3))) exit(2);
            SDL_SetModState(SDL_Keymod(mods));
            SDL_Event event; memset(&event,0,sizeof(event));
            event.type=pressed ? SDL_MOUSEBUTTONDOWN : SDL_MOUSEBUTTONUP;
            event.button.state=pressed ? SDL_PRESSED : SDL_RELEASED;event.button.button=button;
            gamepkg_mouse[button]=pressed!=0;SDL_PushEvent(&event);continue;
        }
        if(!memcmp(packet, "GPW1", 4))
        {
            if(++events > 64) exit(2);
            uint32_t rawx=gamepkg_le32(packet+4),rawy=gamepkg_le32(packet+8),mods=gamepkg_le32(packet+12);
            if((rawx>16 && rawx<0xfffffff0u) || (rawy>16 && rawy<0xfffffff0u) || (mods & ~uint32_t(0x0fc3))) exit(2);
            int x=rawx<=16 ? int(rawx) : -int(~rawx)-1,y=rawy<=16 ? int(rawy) : -int(~rawy)-1;
            if(abs(x)+abs(y)>16) exit(2);
            SDL_SetModState(SDL_Keymod(mods));
            for(int axis=0;axis<2;axis++)
            {
                int steps=axis ? x : y;
                for(int i=0;i<abs(steps);i++)
                {
                    SDL_Event event;memset(&event,0,sizeof(event));event.type=SDL_MOUSEWHEEL;
                    if(axis) event.wheel.x=steps>0 ? 1 : -1;else event.wheel.y=steps>0 ? 1 : -1;
                    SDL_PushEvent(&event);
                }
            }
            continue;
        }
        if(!memcmp(packet, "GPM1", 4))
        {
            if(++events > 64) exit(2);
            uint32_t mods = gamepkg_le32(packet+4);
            if((mods & ~uint32_t(0x0fc3)) || gamepkg_le32(packet+8) || gamepkg_le32(packet+12)) exit(2);
            SDL_SetModState(SDL_Keymod(mods)); continue;
        }
        if(!memcmp(packet, "GPR1", 4))
        {
            if(++events > 64) exit(2);
            if(gamepkg_le32(packet+4) || gamepkg_le32(packet+8) || gamepkg_le32(packet+12)) exit(2);
            for(unsigned i=0;i<6;i++) if(gamepkg_mouse[i])
            {
                SDL_Event event;memset(&event,0,sizeof(event));event.type=SDL_MOUSEBUTTONUP;
                event.button.state=SDL_RELEASED;event.button.button=i;SDL_PushEvent(&event);gamepkg_mouse[i]=false;
            }
            for(unsigned i=4;i<232;i++) if(gamepkg_keys[i])
            {
                SDL_Event event; memset(&event, 0, sizeof(event));
                event.type = SDL_KEYUP; event.key.state = SDL_RELEASED;
                event.key.keysym.scancode = SDL_Scancode(i);
                event.key.keysym.sym = SDL_GetKeyFromScancode(SDL_Scancode(i));
                SDL_PushEvent(&event); gamepkg_keys[i] = false;
            }
            SDL_SetModState(KMOD_NONE);
            continue;
        }
        if(!memcmp(packet, "GPI1", 4))
        {
            if(++events > 64) exit(2);
            uint32_t key = gamepkg_le32(packet+4), flags = gamepkg_le32(packet+8), mods = gamepkg_le32(packet+12);
            if(key < 4 || key > 231 || flags > 3 || (mods & ~uint32_t(0x0fc3))) exit(2);
            SDL_Event event; memset(&event, 0, sizeof(event));
            event.type = flags&1 ? SDL_KEYDOWN : SDL_KEYUP;
            event.key.state = flags&1 ? SDL_PRESSED : SDL_RELEASED;
            event.key.repeat = (flags>>1)&1;
            event.key.keysym.scancode = SDL_Scancode(key);
            event.key.keysym.sym = SDL_GetKeyFromScancode(SDL_Scancode(key));
            event.key.keysym.mod = mods;
            SDL_SetModState(SDL_Keymod(mods));
            gamepkg_keys[key] = (flags&1) != 0;
            SDL_PushEvent(&event);
            continue;
        }
        if(!memcmp(packet, "GPT1", 4))
        {
            if(++events > 64) exit(2);
            uint32_t length = gamepkg_le32(packet+4);
            if(!length || length > 31 || gamepkg_le32(packet+8) || gamepkg_le32(packet+12)) exit(2);
            SDL_Event event; memset(&event, 0, sizeof(event));
            event.type = SDL_TEXTINPUT;
            if(!gamepkg_transfer(0, event.text.text, length, false) || memchr(event.text.text, 0, length)) exit(2);
            SDL_PushEvent(&event);
            continue;
        }
        break;
    }
    if(memcmp(packet, "GPK1", 4)) exit(2);
    const uint32_t buttons = gamepkg_le32(packet+4);
    if(buttons & ~uint32_t(0x3ffff)) exit(2);
    static const SDL_Keycode keys[16] = { SDLK_w, SDLK_s, SDLK_a, SDLK_d,
        SDLK_SPACE, SDLK_LSHIFT, SDLK_RETURN, SDLK_ESCAPE,
        SDLK_TAB, SDLK_e, SDLK_r, SDLK_1, SDLK_2, SDLK_3, SDLK_4, SDLK_5 };
    for(unsigned i=0;i<16;i++) if((buttons ^ gamepkg_previous)&(1u<<i))
    {
        SDL_Event event; memset(&event, 0, sizeof(event));
        event.type = buttons&(1u<<i) ? SDL_KEYDOWN : SDL_KEYUP;
        event.key.state = buttons&(1u<<i) ? SDL_PRESSED : SDL_RELEASED;
        event.key.keysym.sym = keys[i];
        event.key.keysym.scancode = SDL_GetScancodeFromKey(keys[i]);
        SDL_PushEvent(&event);
    }
    for(unsigned i=0;i<2;i++) if((buttons ^ gamepkg_previous)&(1u<<(16+i)))
    {
        SDL_Event event; memset(&event, 0, sizeof(event));
        event.type = buttons&(1u<<(16+i)) ? SDL_MOUSEBUTTONDOWN : SDL_MOUSEBUTTONUP;
        event.button.state = buttons&(1u<<(16+i)) ? SDL_PRESSED : SDL_RELEASED;
        event.button.button = i ? SDL_BUTTON_RIGHT : SDL_BUTTON_LEFT;
        SDL_PushEvent(&event);
    }
    gamepkg_previous = buttons;
    // Bounded relative motion, represented explicitly rather than native structs.
    const uint32_t dx = gamepkg_le32(packet+8), dy = gamepkg_le32(packet+12);
    const int mx = dx <= 2048 ? int(dx) : dx >= 0xfffff800u ? -int(~dx)-1 : 0;
    const int my = dy <= 2048 ? int(dy) : dy >= 0xfffff800u ? -int(~dy)-1 : 0;
    if(mx || my)
    {
        SDL_Event event; memset(&event, 0, sizeof(event));
        event.type = SDL_MOUSEMOTION;
        event.motion.xrel = mx; event.motion.yrel = my;
        SDL_PushEvent(&event);
    }
    gamepkg_running = true;
}
// Fixed 100 ms mono ring. SDL's dummy mixer never opens a host audio device.
static SDL_SpinLock gamepkg_audio_lock = 0;
static int16_t gamepkg_audio_ring[4800];
static unsigned gamepkg_audio_head = 0, gamepkg_audio_count = 0;
static void gamepkg_mix(void *, Uint8 *bytes, int length)
{
    if(length < 0 || length > 1048576 || length%4) return;
    const int16_t *samples = reinterpret_cast<const int16_t *>(bytes);
    SDL_AtomicLock(&gamepkg_audio_lock);
    for(int i=0;i<length/4;i++)
    {
        int16_t mono = (int(samples[i*2])+int(samples[i*2+1]))/2;
        if(gamepkg_audio_count == 4800)
        {
            gamepkg_audio_head = (gamepkg_audio_head+1)%4800;
            gamepkg_audio_count--;
        }
        gamepkg_audio_ring[(gamepkg_audio_head+gamepkg_audio_count)%4800] = mono;
        gamepkg_audio_count++;
    }
    SDL_AtomicUnlock(&gamepkg_audio_lock);
}
void gamepkg_audio_hook()
{
    int frequency, channels; Uint16 format;
    if(!Mix_QuerySpec(&frequency, &format, &channels) || frequency != 48000 ||
       format != AUDIO_S16SYS || channels != 2) exit(2);
    Mix_SetPostMix(gamepkg_mix, NULL);
}
static void gamepkg_present()
{
    if(!gamepkg_running) return;
    int width, height; SDL_GL_GetDrawableSize(screen, &width, &height);
    if(width < 1 || width > 1280 || height < 1 || height > 960) exit(2);
    vector<unsigned char> pixels;
    const size_t length = size_t(width)*size_t(height)*4;
    pixels.reserve(length); pixels.advance(length);
    glPixelStorei(GL_PACK_ALIGNMENT, 1);
    glReadPixels(0, 0, width, height, GL_RGBA, GL_UNSIGNED_BYTE, pixels.getbuf());
    unsigned char pcm[4800*2];
    SDL_AtomicLock(&gamepkg_audio_lock);
    unsigned count = gamepkg_audio_count;
    for(unsigned i=0;i<count;i++)
    {
        uint16_t sample = uint16_t(gamepkg_audio_ring[(gamepkg_audio_head+i)%4800]);
        pcm[i*2] = sample&255; pcm[i*2+1] = sample>>8;
    }
    gamepkg_audio_count = 0; gamepkg_audio_head = 0;
    SDL_AtomicUnlock(&gamepkg_audio_lock);
    unsigned char header[20]; memcpy(header, "GPF2", 4);
    gamepkg_put32(header+4, width); gamepkg_put32(header+8, height);
    gamepkg_put32(header+12, length); gamepkg_put32(header+16, count);
    if(!gamepkg_transfer(gamepkg_output, header, sizeof(header), true) ||
       !gamepkg_transfer(gamepkg_output, pixels.getbuf(), length, true) ||
       !gamepkg_transfer(gamepkg_output, pcm, count*2, true)) exit(0);
}
