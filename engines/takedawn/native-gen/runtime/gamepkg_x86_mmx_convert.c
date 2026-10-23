#include "gamepkg_x86_x87.h"
#include "gamepkg_x86_sse.h"
#include "gamepkg_x86_fault.h"
#include <stdlib.h>
uint64_t gamepkg_x86_convert_mmx(uint64_t value,uint32_t op,uint32_t enter_mmx,uint32_t ip,uint32_t ebp){
    if(op>2 || enter_mmx>1 || (op&& !enter_mmx))abort();
    /* Memory-only CVTPI2PS does not enter MMX or check a pending x87 error.
     * All other forms enter after staged reads, even when #XM suppresses the
     * numerical destination. No physical MMX data is changed until success. */
    if(enter_mmx){gamepkg_x86_x87_wait(ip,ebp);gamepkg_x86_mmx_read_commit();}
    uint32_t flags0,flags1,round=(gamepkg_x86_get_mxcsr()>>13)&3;
    uint32_t lo=gamepkg_x86_convert_bits((uint32_t)value,op,round,&flags0);
    uint32_t hi=gamepkg_x86_convert_bits((uint32_t)(value>>32),op,round,&flags1);
    uint32_t flags=flags0|flags1;
    /* Packed invalid conditions precede precision from either lane. */
    gamepkg_x86_sse_status(flags&1,ip,ebp);
    gamepkg_x86_sse_status(flags&32,ip,ebp);
    return (uint64_t)lo|((uint64_t)hi<<32);
}
