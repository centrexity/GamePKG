/* Maintainer-only backend probe. Writes bounded little-endian native-rate PCM.
 * No player dependency or game API; never publish captured recordings.
 */
#include <mpg123.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <math.h>
#include <fenv.h>
int main(int argc, char **argv)
{
    int float_mode=argc==4 && !strcmp(argv[3],"float");
    if((argc!=3 && !float_mode) || fesetround(FE_TONEAREST) || mpg123_init()!=MPG123_OK) return 1;
    int wanted=float_mode?MPG123_ENC_FLOAT_32:MPG123_ENC_SIGNED_16;
    int error=0;
    mpg123_handle *decoder=mpg123_new(!strcmp(argv[2],"auto")?NULL:argv[2],&error);
    if(!decoder) { fprintf(stderr,"MPG123 error: %s\n",mpg123_plain_strerror(error)); return 2; }
    if(mpg123_format_none(decoder)!=MPG123_OK) return 3;
    const long *rates;size_t rate_count;
    mpg123_rates(&rates,&rate_count);
    for(size_t i=0;i<rate_count;i++)
        if(mpg123_format(decoder,rates[i],MPG123_MONO|MPG123_STEREO,wanted)!=MPG123_OK) return 4;
    if(mpg123_open(decoder,argv[1])!=MPG123_OK) return 5;
    long rate;int channels,encoding;
    if(mpg123_getformat(decoder,&rate,&channels,&encoding)!=MPG123_OK || encoding!=wanted || (channels!=1&&channels!=2)) return 6;
    fprintf(stderr,"MPG123_FORMAT %s %ld %d %d\n",mpg123_current_decoder(decoder),rate,channels,encoding);
    union { uint16_t integer[8192]; float floating[4096]; } samples;
    unsigned char little[sizeof(samples)];size_t total=0;
    const size_t width=float_mode?4u:2u;
    for(;;)
    {
        size_t amount=0;
        int result=mpg123_read(decoder,&samples,sizeof(samples),&amount);
        size_t output_bytes=amount/width*2u;
        if(amount>sizeof(samples) || amount%(width*(unsigned)channels) || output_bytes>64u*1024u*1024u-total) return 7;
        for(size_t i=0;i<amount/width;i++) {
            uint16_t value;
            if(float_mode) {
                float sample=samples.floating[i];
                if(!isfinite(sample)) return 12;
                long quantized=sample>=32767.0f/32768.0f?32767:sample<=-1.0f?-32768:lrintf(sample*32768.0f);
                value=(uint16_t)(int16_t)quantized;
            } else value=samples.integer[i];
            little[2*i]=(unsigned char)value;little[2*i+1]=(unsigned char)(value>>8);
        }
        if(fwrite(little,1,output_bytes,stdout)!=output_bytes) return 8;
        total+=output_bytes;
        if(result==MPG123_DONE) break;
        if(result==MPG123_NEW_FORMAT)
        {
            long next_rate;int next_channels,next_encoding;
            if(mpg123_getformat(decoder,&next_rate,&next_channels,&next_encoding)!=MPG123_OK || next_rate!=rate || next_channels!=channels || next_encoding!=encoding) return 9;
        }
        else if(result!=MPG123_OK) return 10;
    }
    mpg123_close(decoder);mpg123_delete(decoder);mpg123_exit();
    return total>0 && !ferror(stdout) ? 0 : 11;
}
