#ifndef GAMEPKG_MOVIE_IO_H
#define GAMEPKG_MOVIE_IO_H
#include <stdint.h>
// Check each original AVI chunk, including buffered filesystem failures.
template<class Output, class Offset>
bool gamepkg_movie_chunk(Output *output, Offset &total, const char *fourcc,
                         const void *bytes, uint32_t length)
{
    if(output->write(fourcc,4)!=4 || !output->template putlil<uint32_t>(length)
       || output->write(bytes,length)!=length
       || ((length&1u) && !output->putchar(0)) || !output->flush()) return false;
    total+=8u+uint64_t(length)+(length&1u);
    return true;
}
#endif
