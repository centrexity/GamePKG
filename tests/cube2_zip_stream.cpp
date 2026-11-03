#include <algorithm>
#include <cassert>
#include <cstdio>
#include <cstring>
#include <vector>
#include <zlib.h>
#include "zip_bounds.h"
#define STANDALONE
#define DELETEA(p) do { delete[] (p); (p) = NULL; } while(0)
using std::min;
static_assert(__BYTE_ORDER__ == __ORDER_LITTLE_ENDIAN__, "Upstream comparison requires little-endian fixture host");
template<class T> T lilswap(T value) { return value; }
typedef unsigned int uint;
typedef unsigned short ushort;
typedef unsigned char uchar;
template<class T> T clamp(T x, T low, T high) { return std::max(low, std::min(x, high)); }
struct stream { typedef long long offset; };
enum { ZIP_LOCAL_FILE_SIZE = 30, ZIP_LOCAL_FILE_SIGNATURE = 0x04034b50 };
struct ziplocalfileheader {
    uint signature;
    ushort version, flags, compression, modtime, moddate;
    uint crc32, compressedsize, uncompressedsize;
    ushort namelength, extralength;
};
struct zipfile { uint header = 0, offset = ~0U, size = 0, compressedsize = 0; };
struct zipstream;
struct ziparchive { FILE *data; int openfiles = 0; zipstream *owner = NULL; };
// ORIGINAL_LOCAL_HEADER_AND_STREAM
static std::vector<uchar> encode(const std::vector<uchar> &plain)
{
    z_stream z = {};
    assert(deflateInit2(&z, 6, Z_DEFLATED, -MAX_WBITS, 8, Z_DEFAULT_STRATEGY) == Z_OK);
    std::vector<uchar> packed(compressBound(plain.size()));
    z.next_in = const_cast<uchar *>(plain.data()); z.avail_in = plain.size();
    z.next_out = packed.data(); z.avail_out = packed.size();
    assert(deflate(&z, Z_FINISH) == Z_STREAM_END);
    packed.resize(z.total_out);
    assert(deflateEnd(&z) == Z_OK);
    return packed;
}
int main()
{
    std::vector<uchar> plain(40000);
    for(size_t i = 0; i < plain.size(); ++i) plain[i] = uchar((i * 71 + i / 17) % 251);
    for(bool compressed : {false, true}) {
        auto payload = compressed ? encode(plain) : plain;
        FILE *file = tmpfile(); assert(file);
        assert(fwrite(payload.data(), 1, payload.size(), file) == payload.size());
        assert(fflush(file) == 0);
        ziparchive archive; archive.data = file;
        // A prevalidated offset isolates stream reads from local-header parsing.
        for(uint declared : {1u, 3u, 16383u, 16384u, 39999u, 40000u}) {
            for(size_t chunk : {size_t(1), size_t(7), size_t(16384), size_t(50000)}) {
                zipfile entry; entry.offset = 0; entry.size = declared;
                entry.compressedsize = compressed ? payload.size() : 0;
                zipstream input; assert(input.open(&archive, &entry));
                std::vector<uchar> output, buffer(chunk + 16, 0xa5);
                for(;;) {
                    std::fill(buffer.begin(), buffer.end(), 0xa5);
                    // The oversized request must be clamped before zlib narrows it.
                    size_t count = input.read(buffer.data(), chunk == 50000 ? size_t(-1) : chunk);
                    assert(count <= chunk);
                    assert(output.size() + count <= declared);
                    for(size_t i = count; i < buffer.size(); ++i) assert(buffer[i] == 0xa5);
                    output.insert(output.end(), buffer.begin(), buffer.begin() + count);
                    if(!count) break;
                }
                assert(output.size() == declared);
                assert(std::equal(output.begin(), output.end(), plain.begin()));
                assert(input.end());
                assert(input.seek(0, SEEK_SET));
                assert(input.read(buffer.data(), 1) == 1 && buffer[0] == plain[0]);
                assert(input.seek(declared - 1, SEEK_SET));
                assert(input.read(buffer.data(), chunk) == 1 && buffer[0] == plain[declared - 1]);
                assert(input.read(buffer.data(), chunk) == 0);
                input.close(); assert(archive.openfiles == 0);
            }
        }
        assert(fclose(file) == 0);
    }
}
