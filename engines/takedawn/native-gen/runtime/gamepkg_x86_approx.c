#include "gamepkg_x86_approx.h"
#include "approx/gamepkg_approx.h"
#include <stdlib.h>
void gamepkg_x86_approx_f32(const uint32_t *source, uint32_t *result,
                            uint32_t lanes, uint32_t operation) {
  if (!source || !result || (lanes != 1 && lanes != 4) || operation > 1)
    abort();
  uint32_t staged[4];
  for (uint32_t i = 0; i < lanes; ++i)
    staged[i] = source[i];
  for (uint32_t i = 0; i < lanes; ++i)
    staged[i] = gamepkg_approx_f32(staged[i], operation);
  for (uint32_t i = 0; i < lanes; ++i)
    result[i] = staged[i];
}
