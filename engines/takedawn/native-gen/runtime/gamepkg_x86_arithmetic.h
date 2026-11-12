#ifndef GAMEPKG_X86_ARITHMETIC_H
#define GAMEPKG_X86_ARITHMETIC_H
#include <stdint.h>
/* Host-owned binary32 arithmetic, with staged destination writes. Operation
 * 0=add,1=subtract,2=multiply,3=divide,4=square-root(right operand). */
void gamepkg_x86_arithmetic_f32(const uint32_t *left,const uint32_t *right,
    uint32_t *result,uint32_t lanes,uint32_t operation,uint32_t eip,uint32_t ebp);
#endif
