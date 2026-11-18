# NVI1: bounded data for verified vertex/combiner templates

Coordinated with the ports task's renderer review, 2026-11-14. This is the
canonical data representation enforced by `src/shaders/nv2a.rs` and
`graphics::UniformPolicy::Nv2aV1`. It is **not** an implemented/qualified NV2A
interpreter, an approved game profile, or permission to submit shader strings.
The port still supplies the signed bounded Slang templates, raw decoder,
independent evaluator and differential GPU/scene tests. Game translation and
instruction execution do not move into Backend.

## Ownership and layout

The port decodes raw NV2A words/state and emits canonical data. The host installs
verified shader templates through signed Slang preparation, with fixed vertex
layout, binding layout and texture dimensionality. Game requests select those
indices or derive asynchronous generic render-state variants; they never submit
GLSL, WGSL, SPIR-V or GPU objects. Template/source, schema, state and SDK identities
are recorded in cache provenance. A successful structural check is not proof
of correct execution semantics or original-hardware equivalence.

Qualification must pin shader compiler arguments and numerical mode as well as
source/output hashes. Slang `-fp-mode precise` is the agreed preparation
baseline; it alone does not prove that later WGSL/Naga/driver transformations
preserve every floating-point operation across devices. Differential GPU vectors
and scene comparisons still apply. Do not silently switch to fast math.

Use one binding-zero block of **817 uint4 words / 13,072 bytes**. Scalars are
explicit little-endian u32; floating fields contain IEEE-f32 bit patterns. Never
copy a Rust/C native struct or encode usize into the wire. This fits a 16 KiB
uniform-binding limit. Vertex input is sixteen float4 attributes, locations
0–15, offsets `16 * location`, stride 256. The port gathers/defaults all actual
NV2A registers into those positions. Four texture slots each have a fixed 2D or
cube dimensionality in the installed template; sixteen dimension combinations
are possible, not an assertion that every one is used by the title.

The Linux native recipe table fixes pipeline indices 0–15 to the decimal
template suffix and to the four cube-slot bits, one bit per slot. It pins
the corresponding signed Slang files from reviewed ports commit
`de70bb05b3e37058ca4656f2559f34397d573a85`. The recipe provides the base SDR
layout. Use the existing verified state-variant operation for per-draw
raster state. The table contains the NVI1 programs only. SMAA, FXAA,
sharpening and postprocessing are separate stages.

| Byte offset | uint4 words | Field |
| ---: | ---: | --- |
| 0 | 4 | Header/control |
| 64 | 192 | Vertex constants c[192] |
| 3,136 | 5 | Viewport scale, offset, fog parameters/controls, specular controls |
| 3,216 | 544 | Vertex instructions [136], four words per instruction |
| 11,920 | 32 | General combiner stages [8], four words per stage |
| 12,432 | 2 | Final inputs A–G, last lane zero |
| 12,464 | 8 | Per-stage C0 |
| 12,592 | 8 | Per-stage C1 |
| 12,720 | 1 | Final C0 |
| 12,736 | 1 | Final C1 |
| 12,752 | 1 | Fog color |
| 12,768 | 1 | Four texture modes |
| 12,784 | 1 | Four signed-channel masks |
| 12,800 | 1 | Four dependent-source indices |
| 12,816 | 1 | Four clipplane masks |
| 12,832 | 1 | Four alpha-only booleans |
| 12,848 | 4 | Four coordinate-scale vectors |
| 12,912 | 1 | Four texture LOD biases |
| 12,928 | 4 | Four bump matrices |
| 12,992 | 4 | Four bump luminance vectors |
| 13,056 | 1 | HDR parameters; v1 requires SDR bits `(1,1,1,0)` |

Header words:

```text
H0 = (0x3149564e, 1, 13072, final_flags)    // "NVI1"
H1 = (vertex_count, combiner_count, input_mask, modes)
H2 = (unique_c0, unique_c1, mux_msb, fog_input)
H3 = (alpha_ref_f32_bits, alpha_func, alpha_enable, fog_enable)
```

Final flags permit only 0x20 complement R0, 0x40 complement V1, 0x80 clamp sum.
Counts are 1–136 programmed vertex slots and 0–8 combiner stages. `modes[1:0]`
select 0 programmed clip-space, 1 programmed screen-space, 2 fixed pretransformed;
3 is invalid. Fixed requires vertex_count zero and input_mask **0x1E19** (v0,
v3/v4, v9–v12). Programmed masks are recomputed from genuinely consumed input
operands. Mode bit 2 selects the basic modulation fragment path, which requires
combiner_count zero. Other mode bits are zero. H2 lanes and H3 enable lanes are
exactly 0/1. Alpha function is 1–8; reference finite in [0,1], even when disabled.
Inactive instruction/stage suffix records are entirely zero and never execute.

## Vertex records

Every active slot is four uint4s / 64 bytes:

```text
control = (MAC opcode, ILU opcode, MAC destination, ILU destination)
A       = (bank, index, swizzle, flags)
B       = (bank, index, swizzle, flags)
C       = (bank, index, swizzle, flags)
```

Banks: 0 zero, 1 temporary index 0–12, 2 input 0–15, 3 constant 0–191. Swizzles
are eight bits with X/Y/Z/W selectors at bits 7:6 / 5:4 / 3:2 / 1:0; identity
is 0x1B. Flags are bit 0 negate and bit 1 relative constant addressing; relative
is valid only for constants. Unused whole operands and used zero-bank operands
are exactly `(0,0,0,0)`. A consumed constant has a valid base index even when
relative. Compute the active input mask across both units.

Destination: temporary index bits 3:0, temporary mask 7:4, output index 15:8,
output mask 19:16; higher bits zero. Masks retain X=8/Y=4/Z=2/W=1. A zero
temporary mask requires index 15; a nonzero mask requires 0–12. A zero output
mask requires index 255; a nonzero mask requires output 0 or 3–12. The completely
absent destination is **0x0000FF0F**. MAC NOP/ARL and ILU NOP have absent explicit
destinations. At most one unit writes an explicit output register per slot.

MAC 0–13: NOP, MOV, MUL, ADD, MAD, DP3, DPH, DP4, DST, MIN, MAX, SLT, SGE, ARL.
ILU 0–7: NOP, MOV, RCP, RCC, RSQ, EXP, LOG, LIT. Raw reserved encodings and raw
constant-write instructions must fail in the port decoder **before** the legacy
parser can normalize/ignore them. The canonical record cannot prove what raw
source was decoded.

MAC MOV/ARL consume A; ADD consumes A+C; MAD A+B+C; other non-NOP MACs A+B.
Every non-NOP ILU consumes C. Read operands and calculate both results before
writes; commit MAC then ILU with the reference mask/overlap precedence. An active
MAC (including ARL) redirects an enabled ILU temporary write to R1. R12 aliases
oPos. Relative constant selection clamps `a0 + base` to 0–191. Preserve the
adapted signed near-zero-W reconstruction, far-plane rounding guard, fog,
specular, coordinate orientation and the reference limitations on outputs.

ARL execution contract for the **port template**, not implemented by this host
validator:

```text
NaN: unsupported execution; defensively clip this vertex, never cast NaN
+infinity or finite x >= 192: next a0 = 192
-infinity or finite x <= -192: next a0 = -192
otherwise: next a0 = i32(clamp(floor(f32(x + f32(0.001))), -192, 192))
```

Both units use the old a0 before committing the next value. With constant bases
0–191, this saturation preserves finite out-of-range clamped addressing. Supplied
NaNs in constants and typed vertex storage fail host admission. Dynamic NaNs can
still arise during execution, such as zero times an infinite reciprocal. Shape
validation does not prove whole-draw rejection before submission for that case;
claiming it requires sufficient preflight or a proven accepted program/input
restriction. Defensive clipping alone is not that proof.

## Combiner and texture records

Each general stage:

```text
RGB inputs   = (A,B,C,D)
alpha inputs = (A,B,C,D)
RGB output   = (AB destination, CD destination, SUM destination, flags)
alpha output = (AB destination, CD destination, SUM destination, flags)
```

Input descriptors use register bits 3:0, alpha select bit 4, mapping bits 7:5;
higher bits zero. General registers are 0–5 and 8–13; 6/7 are reserved, 14/15
are final-only specials. Destinations use the same general set, with zero
meaning discard. RGB output flags: AB dot3 bit 0, CD dot3 bit 1, mux bit 2,
AB/CD blue-to-alpha bits 3/4, output mapping bits 10:8. Alpha permits only mux
and output mapping. Output mappings are exactly 0/1/2/3/4/6, never reserved 5/7.
Final descriptors allow only identity/invert input mappings; A–D/G may use
specials 14/15, E/F may not. The eighth final-input lane is zero.

The port must qualify simultaneous RGB/alpha reads, AB/CD/SUM write precedence,
blue-to-alpha override, E/F-before-specials evaluation, actual final-flag
meanings, mux quantization and rounded 8-bit alpha discard. No host engine
implementation or approximation is supplied here.

Texture modes 0–7: NONE (0,0,0,1), projective 2D, volume, cube, saturated
coordinate pass-through, clipplane, bump-env, bump-env-luminance. Volume returns
Unsupported until qualified; higher modes Invalid. Cube requires a cube slot;
projective/bump modes require 2D. Bump at stage zero is Unsupported; other bump
sources must name an earlier stage. Signed-channel masks have four bits,
dependent indices 0–3, alpha-only flags 0/1. Coordinate/LOD/bump parameters must
be finite. HDR tail values other than canonical SDR return Unsupported.

Clipplane mask is 0–15 and zero outside clipplane mode. Bit i set discards when
coordinate i >= 0; clear discards when coordinate i < 0. Any failing component
discards; both signed zeros compare >= 0. Mask zero still performs four tests.
The template must preserve signed-channel order, projective q and dependent-
texture semantics; the validator only checks representation/dimensionality.

## Enforcement and qualification

`UniformPolicy::Nv2aV1` requires this exact block size, sixteen-float4 vertex
layout and four matching texture dimensions. Uniform CPU shadows are charged;
validation is cached by dimensionality and invalidated on every write. Vertex
buffers charge a compact per-float NaN bitmap so edits cannot retain a stale
validity result. A rejected block or vertex buffer causes no pass submission,
including no requested attachment clear. Invalid and Unsupported remain distinct.
The pure operation 126 checks the same bytes but cannot waive draw-time checks.

Tests cover positive fixed/programmed records, malformed/truncated blocks,
reserved words/mappings/registers, suffixes, dimensions, supplied NaNs,
mutation/cache invalidation and no partial GPU effects. The GPU layout witness
outputs a constant color; it explicitly **does not** execute NV2A or qualify any
game program. The real precise visibility tests have separate 16,384/zero
reference counts. Read the port review's arithmetic, paired-write, swizzle,
reciprocal, position, texture, alpha and scene vectors before writing templates.

Complete signed Slang vertex/combiner templates, fixed-function/basic branches,
SMAA HIGH, CAS 10%, alternate AA paths and equivalent presentation coordinate
math still need differential preparation and scene qualification. Transcendental
error bounds must be per operation, not a blanket visual tolerance. The actual
native builder/launcher and all source/object/confinement gates must pass before
enabling Burnout. No gameplay, audible audio, physical controls or measured frame
pacing is established by this representation.
