/* Instruction-state rules from Intel SDM CLI/STI operation tables. */
#include "gamepkg_x86_interrupt.h"
#include <stddef.h>
enum { IF = 0x200, VM = 0x20000, VIF = 0x80000, VIP = 0x100000 };
static int valid(const GamePkgX86InterruptState *s) {
  return s && (uintptr_t)s % _Alignof(GamePkgX86InterruptState) == 0 &&
         (uintptr_t)s <= UINTPTR_MAX - (sizeof(*s) - 1) &&
         s->protected_mode <= 1 && s->cpl <= 3 && s->pvi <= 1 && s->vme <= 1 &&
         s->sti_shadow <= 1 && (s->protected_mode || s->cpl == 0) &&
         (!s->protected_mode || !(s->flags & VM) || s->cpl == 3);
}
int gamepkg_x86_interrupt_step(GamePkgX86InterruptState *s, uint32_t op) {
  if (!valid(s) || op > 1)
    return -1;
  uint32_t mask = IF, iopl = (s->flags >> 12) & 3;
  if (s->protected_mode) {
    if (s->flags & VM) {
      if (iopl < 3) {
        if (!s->vme)
          return 1;
        mask = VIF;
      }
    } else if (s->cpl > iopl) {
      if (s->cpl != 3 || !s->pvi)
        return 1;
      mask = VIF;
    }
  }
  if (op && mask == VIF && (s->flags & VIP))
    return 1;
  uint32_t old = s->flags;
  s->flags = op ? old | mask : old & ~mask;
  s->sti_shadow = op && mask == IF && !(old & IF);
  return 0;
}
int gamepkg_x86_interrupt_retire_other(GamePkgX86InterruptState *s) {
  if (!valid(s))
    return -1;
  s->sti_shadow = 0;
  return 0;
}
int gamepkg_x86_interrupt_eligible(const GamePkgX86InterruptState *s) {
  if (!valid(s))
    return -1;
  return !!(s->flags & IF) && !s->sti_shadow;
}
int gamepkg_x86_interrupt_plan_irq(const GamePkgX86InterruptState *s,
                                   uint32_t rep, uint32_t trap,
                                   GamePkgX86IrqFlags *out) {
  if (!valid(s) || !s->protected_mode || s->cpl || (s->flags & VM) || rep > 1 ||
      trap > 1 || !out || (uintptr_t)out % _Alignof(GamePkgX86IrqFlags) ||
      (uintptr_t)out > UINTPTR_MAX - (sizeof(*out) - 1))
    return -1;
  uintptr_t a = (uintptr_t)s, b = (uintptr_t)out;
  if (a <= b ? b - a < sizeof(*s) : a - b < sizeof(*out))
    return -1;
  if (!gamepkg_x86_interrupt_eligible(s))
    return 0;
  const uint32_t tf = 0x100, nt = 0x4000, rf = 0x10000;
  *out =
      (GamePkgX86IrqFlags){s->flags | (rep ? rf : 0),
                           s->flags & ~(tf | nt | rf | VM | (trap ? 0 : IF))};
  return 1;
}
