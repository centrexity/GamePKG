// Bounded original AVI output. Existing per-file quota; no extra frame limit.
#ifndef GAMEPKG_MOVIE_LIMIT_H
#define GAMEPKG_MOVIE_LIMIT_H
#include <stdint.h>
struct gamepkg_movie_budget
{
    bool reached, has_audio;
    gamepkg_movie_budget() : reached(false), has_audio(false) {}
    bool allow(uint64_t written, uint64_t payload, uint64_t projected_indexes, bool audio)
    {
        const uint64_t limit=16u*1048576u;
        if(reached) return false;
        // One RIFF segment: idx1 is 8+16*N; ix00 is 32+8*V;
        // ix01 is 32+8*A when audio exists, with N=V+A.
        // All chunk data is even-padded. Header finalization only seeks.
        if(written>limit || payload>limit || projected_indexes>limit/24u)
        { reached=true; return false; }
        uint64_t footer=40u+24u*projected_indexes+((has_audio||audio) ? 32u : 0u);
        if(written+8u+payload+(payload&1u)+footer>limit)
        { reached=true; return false; }
        has_audio=has_audio||audio;
        return true;
    }
};
#endif
