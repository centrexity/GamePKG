# Original WAD merge source

`upstream/w_merge.c` is unchanged Chocolate DOOM 2.2.1 source from commit
`2ddd290b8226e1d1f0c52e344e76150261a8c0c3`:

https://github.com/chocolate-doom/chocolate-doom/blob/2ddd290b8226e1d1f0c52e344e76150261a8c0c3/src/w_merge.c

Copyright 2005–2014 Simon Howard, GPL-2.0-or-later, as stated in the file.
`upstream-lock.json` pins the exact bytes. Both reviewed engine builders verify
the module before compiling it with `FEATURE_WAD_MERGE`. The reviewed staging step also replaces the pinned feature header’s unconditional undefinition; a compiler flag alone is insufficient.

This revision matches the pinned engine's contiguous `lumpinfo_t` table and
existing `w_merge.h` declarations. Newer Chocolate DOOM revisions use a pointer
table and are not interchangeable. The pinned doomgeneric revision contains
the declarations but omits this implementation.

The module supplies the original merge algorithms. Signed `doom_wad_modes` selects the original grouped loading paths. Native/Wasm resource/map/save qualification passes for `file` and `merge`; mixed file/merge group priority is also verified with the file declared first and last. Generated flat replacement and sprite-only flat exclusion pass all five merge-family modes on native/Wasm. Generated pistol replacement and flat-only sprite exclusion pass merge/sprites/both/flats on native/Wasm. A generated NWT mask-then-file pistol fixture also passes visible replacement, firing and exact idle return on native/Wasm. Directional/world sprites and more complex cases remain separate qualification work. Recipes without this field retain `-file`.
