#ifndef GAMEPKG_X86_STRING_H
#define GAMEPKG_X86_STRING_H
#include <stdint.h>
/* Reviewed host RAM-only implicit strings under flat32 DS/ES, optional FS.
 * op0=MOVS,1=STOS,2=LODS,3=CMPS,4=SCAS; bytes1/2/4; a16/fs boolean.
 * repeat0=single,1=REP/REPE,2=REPNE (compare/scan only). The worker must bind
 * a checked RAM object. Zero repeat count makes no access, even if unbound.
 * Each completed element commits indices/count/data; faulting elements do not.
 * Compare flags commit only on normal exit, retaining pre-instruction flags
 * on a repeated comparison fault. No MMIO, instruction metering, arbitrary
 * memcpy/memmove substitution, interrupt scheduler or interior resume here. */
void gamepkg_x86_string(uint32_t op, uint32_t bytes, uint32_t a16,
                        uint32_t repeat, uint32_t fs, uint32_t eip,
                        uint32_t ebp);
/* Controlled REP interior checkpoints, AFTER an element and only when another
 * element remains/qualifies. Callback0=continue,nonzero=stop; caller supplies
 * reviewed host code, never a guest address. Returns0=whole instruction done,
 * 1=checkpoint stop with next indices/count and current comparison flags.
 * A continued checkpoint restores pre-instruction flags before the next
 * fault-capable element. Faults therefore retain existing rollback semantics.
 * The callback must preserve virtual registers/bindings and return normally.
 * No normal instruction retirement or STI-shadow expiration occurs here.
 * Zero count/single element/final predicate exit have no interior callback.
 */
int gamepkg_x86_string_controlled(uint32_t op, uint32_t bytes, uint32_t a16,
                                  uint32_t repeat, uint32_t fs, uint32_t eip,
                                  uint32_t ebp,
                                  int (*boundary)(uint32_t, uint32_t));
#endif
