#include "../engines/cube2/adapters/movie_limit.h"
#include "../engines/cube2/adapters/movie_io.h"
#include <cassert>
#include <algorithm>
#include <limits>
#include <vector>
#include <cstdio>

struct file_output
{
    FILE *file;
    size_t write(const void *bytes,size_t length) { return fwrite(bytes,1,length,file); }
    template<class T> bool putlil(T value)
    {
        unsigned char encoded[sizeof(T)];
        for(size_t i=0;i<sizeof(T);++i) encoded[i]=static_cast<unsigned char>(value>>(8*i));
        return write(encoded,sizeof(T))==sizeof(T);
    }
    bool putchar(int value) { return fputc(value,file)!=EOF; }
    bool flush() { return fflush(file)==0; }
};

struct failing_output
{
    std::vector<unsigned char> bytes;
    size_t available;
    bool flush_ok;
    failing_output(size_t capacity, bool flush=true) : available(capacity), flush_ok(flush) {}
    size_t write(const void *data, size_t length)
    {
        size_t accepted=std::min(length,available);
        const unsigned char *source=static_cast<const unsigned char *>(data);
        bytes.insert(bytes.end(),source,source+accepted);
        available-=accepted;
        return accepted;
    }
    template<class T> bool putlil(T value)
    {
        unsigned char encoded[sizeof(T)];
        for(size_t i=0;i<sizeof(T);++i) encoded[i]=static_cast<unsigned char>(value>>(8*i));
        return write(encoded,sizeof(T))==sizeof(T);
    }
    bool putchar(int value) { unsigned char byte=value; return write(&byte,1)==1; }
    bool flush() { return flush_ok; }
};

int main()
{
    const unsigned char samples[]={1,2,3};
    const std::vector<unsigned char> expected={'0','1','w','b',3,0,0,0,1,2,3,0};
    for(size_t cut=0;cut<expected.size();++cut)
    {
        failing_output output(cut);
        uint64_t total=123;
        assert(!gamepkg_movie_chunk(&output,total,"01wb",samples,3));
        assert(total==123 && output.bytes.size()==cut);
        assert(std::equal(output.bytes.begin(),output.bytes.end(),expected.begin()));
    }
    failing_output buffered(100,false);
    uint64_t total=123;
    assert(!gamepkg_movie_chunk(&buffered,total,"01wb",samples,3));
    assert(total==123);
    failing_output complete(100);
    assert(gamepkg_movie_chunk(&complete,total,"01wb",samples,3));
    assert(total==135 && complete.bytes==expected);
    // Real buffered ENOSPC: fwrite may succeed, but fflush must be observed.
    file_output disk={fopen("/dev/full","wb")};
    assert(disk.file);
    total=123;
    assert(!gamepkg_movie_chunk(&disk,total,"01wb",samples,3));
    assert(total==123 && ferror(disk.file));
    fclose(disk.file);
    const uint64_t cap=16u*1048576u;
    // Exact final size includes the chunk header and both index formats.
    gamepkg_movie_budget exact;
    assert(exact.allow(cap-8-100-64,100,1,false));
    assert(!exact.allow(cap-8-100-64+1,100,1,false));
    assert(exact.reached);
    assert(!exact.allow(0,0,0,false));
    gamepkg_movie_budget odd;
    assert(odd.allow(cap-8-102-64,101,1,false));
    assert(!odd.allow(cap-8-102-64+1,101,1,false));
    gamepkg_movie_budget audio;
    assert(audio.allow(cap-8-100-96,100,1,true));
    assert(audio.has_audio);
    assert(!audio.allow(cap-8-100-96+1,100,1,false));
    gamepkg_movie_budget late_audio;
    assert(late_audio.allow(100,100,1,false));
    assert(!late_audio.allow(cap-8-100-64,100,1,true));
    // Duplicate video-frame indexes consume space even without extra payload.
    gamepkg_movie_budget duplicates;
    assert(!duplicates.allow(cap-8-100-64,100,2,false));
    for(int field=0;field<3;++field)
    {
        gamepkg_movie_budget overflow;
        const uint64_t huge=std::numeric_limits<uint64_t>::max();
        assert(!overflow.allow(field==0?huge:0,field==1?huge:0,field==2?huge:0,false));
    }
    gamepkg_movie_budget stream;
    uint64_t written=1024, count=0;
    while(stream.allow(written,4095,count+1,(count%3)==0))
    {
        written+=8+4096;
        ++count;
        assert(written+40+24*count+(stream.has_audio?32:0)<=cap);
    }
    assert(count>1000 && written>15u*1048576u && stream.reached);
}
