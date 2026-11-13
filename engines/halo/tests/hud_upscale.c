/* Asset-free fixture; reference kernel is recovered from its pinned reviewed diff. */
#include <fenv.h>
#include <stdio.h>
#include <time.h>
static uint64_t seed=UINT64_C(0x9e3779b97f4a7c15);
static uint32_t random32(void){seed^=seed<<13;seed^=seed>>7;seed^=seed<<17;return(uint32_t)seed;}
static unsigned cases;
static void compare(const unsigned char *source,unsigned long width,unsigned long height)
{
    size_t size=width*height*64;
    feclearexcept(FE_ALL_EXCEPT);
    unsigned char *expected=reference_pixels(source,width,height);
    int flags=fetestexcept(FE_ALL_EXCEPT);
    feclearexcept(FE_ALL_EXCEPT);
    unsigned char *actual=hud_hires_upscale_pixels(source,width,height);
    int got_flags=fetestexcept(FE_ALL_EXCEPT);
    if(!expected||!actual||memcmp(expected,actual,size)||flags!=got_flags)
    {
        size_t byte=0;
        while(expected&&actual&&byte<size&&expected[byte]==actual[byte])byte++;
        fprintf(stderr,"case%u %lux%lu rounding%d byte%zu expected%d actual%d flags%x/%x\n",cases,width,height,fegetround(),byte,expected&&byte<size?expected[byte]:-1,actual&&byte<size?actual[byte]:-1,flags,got_flags);
        abort();
    }
    free(expected);free(actual);cases++;
}
static uint64_t ns(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return(uint64_t)t.tv_sec*1000000000+(uint64_t)t.tv_nsec;}
int main(void)
{
    unsigned char pixel[4]={0,0,0,0};
    const int modes[]={FE_TONEAREST,FE_UPWARD,FE_DOWNWARD,FE_TOWARDZERO};
    const unsigned dimensions[]={1,2,3,4,5,7,8,9,15,16,17,31,32,33,63,64,65,127,128,129,255,256};
    for(unsigned mode=0;mode<4;mode++)
    {
        if(fesetround(modes[mode]))abort();
        for(unsigned color=0;color<256;color++)
        {
            pixel[0]=(unsigned char)color;pixel[1]=(unsigned char)(255-color);
            pixel[2]=(unsigned char)(color^0x55);pixel[3]=(unsigned char)(color*73);
            compare(pixel,1,1);
        }
        for(unsigned a=0;a<sizeof(dimensions)/sizeof(*dimensions);a++)
        {
            unsigned width=dimensions[a],height=dimensions[(a*7+mode)%22];
            size_t size=(size_t)width*height*4,offset=1+(random32()%15);
            unsigned char *allocation=malloc(size+offset),*source=allocation+offset;
            for(size_t i=0;i<size;i++)source[i]=(unsigned char)random32();
            compare(source,width,height);free(allocation);
        }
        unsigned char taps[64];
        unsigned patterns=getenv("GAMEPKG_HUD_QUICK")?1024:65536;
        for(unsigned pattern=0;pattern<patterns;pattern++)
        {
            for(unsigned i=0;i<16;i++)
            {
                unsigned char value=(pattern&(1u<<i))?255:0;
                taps[i*4]=value;taps[i*4+1]=(unsigned char)(255-value);
                taps[i*4+2]=value;taps[i*4+3]=(unsigned char)(value?127:128);
            }
            compare(taps,4,4);
        }
    }
    for(unsigned width=0;width<2;width++)
    {
        if(hud_hires_upscale_pixels(NULL,width,1)||hud_hires_upscale_pixels(pixel,0,1)||
            hud_hires_upscale_pixels(pixel,1,0)||hud_hires_upscale_pixels(pixel,257,1)||hud_hires_upscale_pixels(pixel,1,257))abort();
    }
    fesetround(FE_TONEAREST);
    unsigned char *source=malloc(256*256*4);
    for(unsigned i=0;i<256*256*4;i++)source[i]=(unsigned char)random32();
    uint64_t start=ns();
    for(unsigned i=0;i<8;i++){unsigned char *p=reference_pixels(source,256,256);free(p);}
    uint64_t old_ns=ns()-start;start=ns();
    for(unsigned i=0;i<8;i++){unsigned char *p=hud_hires_upscale_pixels(source,256,256);free(p);}
    uint64_t new_ns=ns()-start;free(source);
    printf("%u bit-exact image cases and FP flags across4roundingmodes; reference%.3fms optimized%.3fms per256x256\n",cases,old_ns/8e6,new_ns/8e6);
    return 0;
}
