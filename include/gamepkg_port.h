#ifndef GAMEPKG_PORT_H
#define GAMEPKG_PORT_H
/* Game-facing, versioned port ABI. No SDL, platform or GPU types.
 * All request/reply integers and IEEE floats are little endian byte fields,
 * not packed C structs. Every resource/grant ID is a host-issued uint64_t.
 * Transport/bootstrap functions are private to the GamePKG launcher.
 * The host must confine the game process; linking this client is not itself
 * protection against arbitrary native machine code. See docs/PORT_SERVICES.md.
 */
#include <stdint.h>
#include <stddef.h>
/* Direct-compiled SIMD backend selection stays inside this GamePKG-owned
 * header. Game code calls fixed-width helpers and never sees CPUID, target
 * architecture queries, pointers or platform intrinsics. */
#if defined(__AVX2__)
#include <immintrin.h>
#define GAMEPKG_SIMD_X86_AVX2 1
#endif
#if defined(__SSE2__) || defined(__x86_64__)
#include <emmintrin.h>
#define GAMEPKG_SIMD_X86_SSE2 1
#endif
#if defined(__aarch64__)
#include <arm_neon.h>
#define GAMEPKG_SIMD_AARCH64_NEON 1
#endif
#define GAMEPKG_SIMD_ABI 2u
#define GAMEPKG_PORT_ABI 1u
#define GAMEPKG_PORT_MAX_MESSAGE (16u*1024u*1024u)
#define GAMEPKG_PORT_INVALID -1
#define GAMEPKG_PORT_BUSY -2
#define GAMEPKG_PORT_FAILED -3
#define GAMEPKG_PORT_DENIED -4
#define GAMEPKG_PORT_UNSUPPORTED -5
#define GAMEPKG_PORT_COMMITTED_NOT_DURABLE -6
#define GAMEPKG_PORT_VERSION 0u
#define GAMEPKG_PORT_FILE_OPEN 1u
#define GAMEPKG_PORT_FILE_MODE_READ 0u
#define GAMEPKG_PORT_FILE_MODE_READ_WRITE_CREATE 1u
#define GAMEPKG_PORT_FILE_MODE_READ_WRITE_EXISTING 2u
#define GAMEPKG_PORT_FILE_MODE_CREATE_NEW 3u
#define GAMEPKG_PORT_FILE_READ 2u
#define GAMEPKG_PORT_FILE_WRITE 3u
#define GAMEPKG_PORT_FILE_CLOSE 4u
#define GAMEPKG_PORT_FILE_SIZE 5u
#define GAMEPKG_PORT_DIRECTORY_LIST 6u
#define GAMEPKG_PORT_DIRECTORY_CREATE 7u
#define GAMEPKG_PORT_FILE_REMOVE 8u
#define GAMEPKG_PORT_FILE_RESIZE 9u
#define GAMEPKG_PORT_WINDOW_COMMAND 10u
#define GAMEPKG_PORT_WINDOW_STATE 11u
#define GAMEPKG_PORT_FILE_SYNC 12u
#define GAMEPKG_PORT_GRANTS 13u
#define GAMEPKG_PORT_FILE_REPLACE 14u
#define GAMEPKG_PORT_FILE_METADATA 15u
#define GAMEPKG_PORT_FILE_DUPLICATE 16u
#define GAMEPKG_PORT_FILE_EXISTS 17u
#define GAMEPKG_PORT_ROM_VIEW 19u
#define GAMEPKG_PORT_DIRECTORY_REMOVE 18u
#define GAMEPKG_PORT_MONOTONIC 60u
#define GAMEPKG_PORT_WAIT_UNTIL 61u
#define GAMEPKG_PORT_WAKE 62u
#define GAMEPKG_PORT_DIAGNOSTIC 63u /* level/code/count u32, <=32 values u64; sequence u64 */
#define GAMEPKG_PORT_DIAGNOSTIC_BYTES 64u /* level/code/byte-count u32, <=128 KiB bytes; sequence u64 */
#define GAMEPKG_PORT_LAUNCH_SETTINGS 67u /* empty request, eight option bytes; 255 means unspecified */
#define GAMEPKG_PORT_RUMBLE_FRAME 57u /* player/low/high u32, frame u64; one numbered VI field */
#define GAMEPKG_PORT_VI_CADENCE 58u /* standard u32; period u64, standard/fields u32 */
#define GAMEPKG_PORT_FONT_GLYPH 75u /* font/codepoint/size u32; width/height/advance u32 + RGBA8 */
#define GAMEPKG_PORT_J3D_STATE 156u /* template u32 + 48 state words; stable 32-byte identity */
#define GAMEPKG_PORT_J3D_RECALL 157u /* 32-byte identity; 48 canonical state words */
#define GAMEPKG_PORT_THP_FRAME 158u /* standard u32, start/field u64; field/deadline/callbacks */
#define GAMEPKG_PORT_THP_DISPLAY 159u /* width/height u32 + YUV420 planes; RGBA8 */
#define GAMEPKG_PORT_CLOCK_SAMPLE 65u /* empty request; 4 LE u64: counter/frequency/uptime-ms/UTC100ns-since-1601; worker only */
#define GAMEPKG_PORT_PERFORMANCE_SAMPLE 66u /* empty request; 3 LE u64: counter/frequency/uptime-ms; worker only; never anchors calendar */
#define GAMEPKG_PORT_IMAGE_CREATE 20u
#define GAMEPKG_PORT_IMAGE_RELEASE 21u
#define GAMEPKG_PORT_PRESENT 22u
#define GAMEPKG_PORT_SAVE_VOLUME 23u
#define GAMEPKG_PORT_SAVE_DIRECTORY 24u
#define GAMEPKG_PORT_SAVE_FILE_INFO 25u
#define GAMEPKG_PORT_SAVE_FILE_SET_ATTRIBUTES 26u
#define GAMEPKG_PORT_AUDIO 30u
/* Four LE u32: format (0 PCM16), source rate (Hz), layout (0 mono / 1 stereo /
 * 2 explicit 5.1), capacity in source frames. Reply: LE u64 stream ID.
 * Rates: 22050, 24000, 32000, 44100, 48000. Others return UNSUPPORTED. */
#define GAMEPKG_PORT_AUDIO_STREAM_CREATE 130u
#define GAMEPKG_PORT_AUDIO_STREAM_SUBMIT 131u
#define GAMEPKG_PORT_AUDIO_STREAM_STATUS 132u
#define GAMEPKG_PORT_AUDIO_STREAM_CONTROL 133u
#define GAMEPKG_PORT_AUDIO_STREAM_RELEASE 134u
#define GAMEPKG_PORT_AUDIO_STREAM_GAIN 136u
#define GAMEPKG_PORT_AUDIO_RECORD_CREATE 137u
#define GAMEPKG_PORT_AUDIO_RECORD_READ 138u
#define GAMEPKG_PORT_AUDIO_RECORD_RELEASE 139u
#define GAMEPKG_PORT_VIEWPORT_PASS 129u
/* Operation 135 takes the 36-byte 122 sampler descriptor followed by one LE
 * u32 border color: 0 transparent black, 1 opaque black, 2 opaque white.
 * Address mode 3 is ClampToBorder on any axis and requires host support. */
#define GAMEPKG_PORT_BORDER_SAMPLER_CREATE 135u
#define GAMEPKG_PORT_CAPTURE_CREATE 140u
#define GAMEPKG_PORT_CAPTURE_READ 141u
#define GAMEPKG_PORT_CAPTURE_STATUS 142u
#define GAMEPKG_PORT_CAPTURE_RELEASE 143u
#define GAMEPKG_LOCAL_MAX_MESSAGE (1024u*1024u)
#define GAMEPKG_GRANT_ASSETS 1u
#define GAMEPKG_GRANT_SAVE 2u
#define GAMEPKG_GRANT_ROM 3u
#define GAMEPKG_GRANT_RAM_CACHE 4u
#define GAMEPKG_GRANT_PREVIOUS_SAVE 5u
#define GAMEPKG_GRANT_GAME_DATA 6u
#define GAMEPKG_LOCAL_VERSION 0u
#define GAMEPKG_LOCAL_WINDOWS 1u
#define GAMEPKG_LOCAL_COMMIT 2u
#define GAMEPKG_LOCAL_PROTECT 3u
#define GAMEPKG_LOCAL_DECOMMIT 4u
#define GAMEPKG_LOCAL_READ 5u
#define GAMEPKG_LOCAL_WRITE 6u
#define GAMEPKG_LOCAL_SPAWN 10u
#define GAMEPKG_LOCAL_JOIN 11u
#define GAMEPKG_LOCAL_CANCEL 12u
#define GAMEPKG_LOCAL_RESUME 13u
#define GAMEPKG_LOCAL_CURRENT_TASK 14u
#define GAMEPKG_LOCAL_PAGE_STATES 17u
#define GAMEPKG_LOCAL_MAX_QUERY_PAGES 256u
#define GAMEPKG_LOCAL_PAGE_UNCOMMITTED 0u
#define GAMEPKG_LOCAL_PAGE_NO_ACCESS 1u
#define GAMEPKG_LOCAL_PAGE_READ_ONLY 2u
#define GAMEPKG_LOCAL_PAGE_READ_WRITE 3u
#define GAMEPKG_LOCAL_EVENT 20u
#define GAMEPKG_LOCAL_MUTEX 21u
#define GAMEPKG_LOCAL_SIGNAL 22u
#define GAMEPKG_LOCAL_WAIT 23u
#define GAMEPKG_LOCAL_UNLOCK 24u
#define GAMEPKG_LOCAL_RELEASE_SYNC 25u
#define GAMEPKG_LOCAL_MUTEX_LOCK 26u /* mutex ID u64 + timeout ms u32 (0..1000); empty reply; events reject without consumption */
#define GAMEPKG_LOCAL_EVENT_WAIT 27u /* event ID u64 + timeout ms u32 (0..1000); empty reply; cancellable, never acquires a mutex */
#define GAMEPKG_LOCAL_EVENT_STATE 28u /* event ID u64; LE u32 manual + u32 signaled; non-consuming, no lifetime extension */
#define GAMEPKG_LOCAL_SYNC_RELEASE 49u /* ID u64 + actual kind u32 (1 event/2 mutex/3 semaphore); empty reply; typed cancelled-task cleanup, held mutex Busy */
#define GAMEPKG_LOCAL_MUTEX_STATE 48u /* ID u64; LE u32 locked/owned_by_caller/abandoned/depth; non-consuming, no ownership or lifetime grant */
#define GAMEPKG_LOCAL_WAIT_MANY 47u /* count/all/ms u32, then 1..64 (ID u64, kind u32); index/reason u32, reason 1=acquired abandoned mutex */
#define GAMEPKG_LOCAL_WAIT_EVENT 1u
#define GAMEPKG_LOCAL_WAIT_MUTEX 2u
#define GAMEPKG_LOCAL_WAIT_SEMAPHORE 3u
#define GAMEPKG_LOCAL_WAIT_TASK 4u
#define GAMEPKG_LOCAL_SEMAPHORE_CREATE 43u /* LE u32 initial + maximum (1..INT32_MAX); opaque ID u64 */
#define GAMEPKG_LOCAL_SEMAPHORE_RELEASE 44u /* ID u64 + positive permits u32; previous count u32, atomic */
#define GAMEPKG_LOCAL_SEMAPHORE_WAIT 45u /* ID u64 + timeout ms u32 (0..1000); typed, cancellable; empty reply */
#define GAMEPKG_LOCAL_SEMAPHORE_STATE 46u /* ID u64; count + maximum u32; copied observation, no lease */
/* Empty request; exactly counter/frequency/uptime_ms, three LE u64. */
#define GAMEPKG_LOCAL_PERFORMANCE_SAMPLE 38u
#define GAMEPKG_LOCAL_CLOCK_STATE 39u /* empty; counter/frequency/uptime-ms/generation, four LE u64 */
#define GAMEPKG_LOCAL_CLOCK_WAIT 40u /* deadline/frequency/generation, three LE u64; state + LE u32 reason */
#define GAMEPKG_LOCAL_CLOCK_WAKE 41u /* empty; nonzero generation LE u64; one timing lane per runtime */
#define GAMEPKG_NETWORK_INTERFACES 14u
#define GAMEPKG_PORT_INPUT_EVENTS 40u
#define GAMEPKG_PORT_GAMEPAD_STATE 41u
#define GAMEPKG_PORT_KEY_STATE 42u
#define GAMEPKG_PORT_POINTER_STATE 43u
#define GAMEPKG_PORT_TOUCH_CONTACTS 44u
#define GAMEPKG_PORT_RUMBLE 45u
#define GAMEPKG_PORT_INPUT_DEVICES 46u
#define GAMEPKG_PORT_NETWORK 50u
#define GAMEPKG_PORT_BUFFER_CREATE 100u
#define GAMEPKG_PORT_BUFFER_WRITE 101u
#define GAMEPKG_PORT_TEXTURE_CREATE 102u
#define GAMEPKG_PORT_TEXTURE_WRITE 103u
#define GAMEPKG_PORT_GPU_RELEASE 104u
#define GAMEPKG_PORT_DRAW_INDEXED 105u
#define GAMEPKG_PORT_TARGET_CREATE 106u
#define GAMEPKG_PORT_TARGET_COPY 107u
#define GAMEPKG_PORT_TARGET_PASS 108u
#define GAMEPKG_PORT_TARGET_PRESENT 109u
#define GAMEPKG_PORT_TARGET_CLEAR_RECT 110u
#define GAMEPKG_PORT_VISIBILITY_CREATE 111u
#define GAMEPKG_PORT_VISIBILITY_POLL 113u
#define GAMEPKG_PORT_TARGET_QUERY_PASS 115u
#define GAMEPKG_PORT_TARGET_COPY_TEXTURE 116u
#define GAMEPKG_PORT_DEPTH_CREATE 118u
#define GAMEPKG_PORT_TARGET_DEPTH 119u
#define GAMEPKG_PORT_TARGET_CLEAR_STENCIL 120u
#define GAMEPKG_PORT_SAMPLER_CREATE 122u
#define GAMEPKG_PORT_TARGET_STATE_PASS 123u
#define GAMEPKG_PORT_TARGET_READBACK 124u
#define GAMEPKG_PORT_READBACK_POLL 125u
#define GAMEPKG_PORT_NVI1_VALIDATE 126u
#define GAMEPKG_PORT_PIPELINE_PREPARE 127u
#define GAMEPKG_PORT_PIPELINE_POLL 128u
#define GAMEPKG_PORT_VERTEX_BUFFER 0u
#define GAMEPKG_PORT_INDEX_BUFFER 1u
#define GAMEPKG_PORT_UNIFORM_BUFFER 2u
#define GAMEPKG_PORT_SAMPLER_LINEAR 1u
#define GAMEPKG_PORT_SAMPLER_REPEAT_U 2u
#define GAMEPKG_PORT_SAMPLER_MIRROR_U 4u
#define GAMEPKG_PORT_SAMPLER_REPEAT_V 8u
#define GAMEPKG_PORT_SAMPLER_MIRROR_V 16u
#define GAMEPKG_PORT_SAMPLER_REPEAT_W 32u
#define GAMEPKG_PORT_SAMPLER_MIRROR_W 64u
#ifdef __cplusplus
extern "C" {
#endif
/* Trusted child-local UTF-8 validator used by generated bindings. Buffers must
 * remain readable/immutable; no pointers cross the private service transport. */
/* mode 0 validates UTF-8; mode 1 also rejects empty/dot components,
 * separators and Unicode controls. Unknown modes are Invalid. */
int32_t gamepkg_port_utf8_validate(const uint8_t *bytes, uint32_t length, uint32_t mode);
/* Nonnegative: reply byte count. Negative: one of the status values above.
 * The trusted client serializes concurrent callers. BUSY is retryable;
 * transport failure permanently disconnects the client, with no fallback to
 * game-owned OS APIs. Buffers must be valid for their declared length.
 */
int32_t gamepkg_port_call(uint32_t operation, const uint8_t *request,
    uint32_t length, uint8_t *reply, uint32_t capacity);
/* Child-local trusted runtime, a separate namespace; never a remote syscall.
 * Bootstrap fixes windows/tasks. The game receives no native pointers/handles.
 * Local 9 gather / 15 scatter: LE count u32 (1..32), then count descriptors
 * (opaque window ID, offset, positive byte count: three u64). Gather ends at
 * the header and returns concatenated bytes. Scatter appends exact payload;
 * later aliased destinations win. Total payload <=1 MiB; complete request and
 * reply each <=1 MiB. All ranges/rights are checked before any access/mutation.
 * These are RAM copies inside this confined process, never device/OS access.
 * Local 16 task finished: exact LE task ID u64; result bool u32 (0/1).
 * Non-consuming snapshot; no wait, resume, cancellation, join result or OS ID.
 * Local 17 page states: exact LE window ID u64, offset u64, page count u32,
 * reserved-zero u32. Offset is 4 KiB aligned, count 1..256. Reply is count bytes:
 * 0 uncommitted, 1 committed/no-access, 2 read-only, 3 read/write. Copied under
 * the memory lock; no page/data/protection mutation or native address disclosure.
 * The snapshot does not retain memory rights or guarantee later access.
 * Query states differ from Local 3 protection modes (0 no-access, 1 RO, 2 RW).
 */
int32_t gamepkg_local_call(uint32_t operation,const uint8_t *request,
    uint32_t length,uint8_t *reply,uint32_t capacity);
#ifdef __cplusplus
}
#endif
static inline void gamepkg_port_put_u32(uint8_t *p, uint32_t v) {
    p[0]=(uint8_t)v; p[1]=(uint8_t)(v>>8); p[2]=(uint8_t)(v>>16); p[3]=(uint8_t)(v>>24);
}
static inline uint32_t gamepkg_port_get_u32(const uint8_t *p) {
    return (uint32_t)p[0]|((uint32_t)p[1]<<8)|((uint32_t)p[2]<<16)|((uint32_t)p[3]<<24);
}
static inline void gamepkg_port_put_u64(uint8_t *p, uint64_t v) {
    gamepkg_port_put_u32(p,(uint32_t)v);gamepkg_port_put_u32(p+4,(uint32_t)(v>>32));
}
static inline uint64_t gamepkg_port_get_u64(const uint8_t *p) {
    return (uint64_t)gamepkg_port_get_u32(p)|((uint64_t)gamepkg_port_get_u32(p+4)<<32);
}
/* One exact motor pulse for one numbered VI field. Replayed frames are rejected. */
static inline int32_t gamepkg_rumble_frame(uint32_t player,uint16_t low,uint16_t high,uint64_t frame) {
    uint8_t request[20];
    if (player >= 4u || frame == 0u) return GAMEPKG_PORT_INVALID;
    gamepkg_port_put_u32(request,player);
    gamepkg_port_put_u32(request+4,(uint32_t)low);
    gamepkg_port_put_u32(request+8,(uint32_t)high);
    gamepkg_port_put_u64(request+12,frame);
    return gamepkg_port_call(GAMEPKG_PORT_RUMBLE_FRAME,request,20,NULL,0);
}
/* Host-pinned VI period; no wall-clock or host-display calibration. */
static inline int32_t gamepkg_vi_cadence(uint32_t standard,uint64_t *period_ns,uint32_t *fields_per_frame) {
    uint8_t reply[16];
    int32_t result;
    if (period_ns == NULL || fields_per_frame == NULL) return GAMEPKG_PORT_INVALID;
    result = gamepkg_port_call(GAMEPKG_PORT_VI_CADENCE,(const uint8_t *)&standard,4,reply,16);
    if (result != 16) return result < 0 ? result : GAMEPKG_PORT_FAILED;
    *period_ns = gamepkg_port_get_u64(reply);
    if (gamepkg_port_get_u32(reply+8) != standard) return GAMEPKG_PORT_FAILED;
    *fields_per_frame = gamepkg_port_get_u32(reply+12);
    if (*period_ns == 0u || *fields_per_frame == 0u) return GAMEPKG_PORT_FAILED;
    return 16;
}
/* Host-owned bitmap glyph. reply must have 12 + 5*size*7*size*4 bytes. The
 * first three LE u32 words are width, height and advance; RGBA8 follows. */
static inline int32_t gamepkg_font_glyph(uint32_t font,uint32_t codepoint,uint32_t size,uint8_t *reply,uint32_t capacity) {
    uint8_t request[12];
    uint32_t needed;
    int32_t result;
    if (font != 0u || size == 0u || size > 32u || reply == NULL) return GAMEPKG_PORT_INVALID;
    needed = 12u + (5u*size)*(7u*size)*4u;
    if (capacity < needed) return GAMEPKG_PORT_INVALID;
    gamepkg_port_put_u32(request,font);
    gamepkg_port_put_u32(request+4,codepoint);
    gamepkg_port_put_u32(request+8,size);
    result = gamepkg_port_call(GAMEPKG_PORT_FONT_GLYPH,request,12,reply,needed);
    if (result < 0) return result;
    if ((uint32_t)result != needed || gamepkg_port_get_u32(reply) != 5u*size
        || gamepkg_port_get_u32(reply+4) != 7u*size) return GAMEPKG_PORT_FAILED;
    return result;
}
/* Host-reviewed J3D/GX state identity and exact canonical recall. */
static inline int32_t gamepkg_j3d_state(const uint32_t state[48],uint32_t template_index,uint8_t identity[32]) {
    uint8_t request[196];
    uint32_t i;
    int32_t result;
    if (state == NULL || identity == NULL) return GAMEPKG_PORT_INVALID;
    gamepkg_port_put_u32(request,template_index);
    for (i=0;i<48u;++i) gamepkg_port_put_u32(request+4+i*4,state[i]);
    result = gamepkg_port_call(GAMEPKG_PORT_J3D_STATE,request,196,identity,32);
    return result == 32 ? 32 : (result < 0 ? result : GAMEPKG_PORT_FAILED);
}
static inline int32_t gamepkg_j3d_recall(const uint8_t identity[32],uint32_t state[48]) {
    uint8_t reply[192];
    uint32_t i;
    int32_t result;
    if (identity == NULL || state == NULL) return GAMEPKG_PORT_INVALID;
    result = gamepkg_port_call(GAMEPKG_PORT_J3D_RECALL,identity,32,reply,192);
    if (result < 0) return result;
    if (result != 192) return GAMEPKG_PORT_FAILED;
    for (i=0;i<48u;++i) state[i]=gamepkg_port_get_u32(reply+i*4);
    return 192;
}
/* THP frame cadence and fixed-point GX/TEV YUV presentation. */
static inline int32_t gamepkg_thp_frame(uint32_t standard,uint64_t start_ns,uint64_t field,uint64_t *deadline_ns,uint32_t *callbacks) {
    uint8_t request[20],reply[24];
    int32_t result;
    if (deadline_ns == NULL || callbacks == NULL) return GAMEPKG_PORT_INVALID;
    gamepkg_port_put_u32(request,standard);
    gamepkg_port_put_u64(request+4,start_ns);
    gamepkg_port_put_u64(request+12,field);
    result = gamepkg_port_call(GAMEPKG_PORT_THP_FRAME,request,20,reply,24);
    if (result < 0) return result;
    if (result != 24 || gamepkg_port_get_u64(reply)!=field
        || gamepkg_port_get_u32(reply+20)!=0u) return GAMEPKG_PORT_FAILED;
    *deadline_ns = gamepkg_port_get_u64(reply+8);
    *callbacks = gamepkg_port_get_u32(reply+16);
    if (*callbacks == 0u) return GAMEPKG_PORT_FAILED;
    return 24;
}
/* planes is one contiguous Y plane followed by U and V quarter planes.
 * request must have 8 + 1.5*luma writable bytes and is reused by the helper. */
static inline int32_t gamepkg_thp_display(uint32_t width,uint32_t height,const uint8_t *planes,uint8_t *request,uint32_t request_capacity,uint8_t *rgba,uint32_t capacity) {
    uint32_t luma,chroma,needed,request_bytes,offset;
    int32_t result;
    if (width==0u || height==0u || width>2048u || height>2048u
        || (width&1u)!=0u || (height&1u)!=0u || planes==NULL || request==NULL || rgba==NULL) return GAMEPKG_PORT_INVALID;
    luma = width*height;
    chroma = luma/4u;
    needed = luma*4u;
    request_bytes = 8u+luma+chroma*2u;
    if (capacity < needed || request_capacity < request_bytes
        || request_bytes > GAMEPKG_PORT_MAX_MESSAGE) return GAMEPKG_PORT_INVALID;
    gamepkg_port_put_u32(request,width);
    gamepkg_port_put_u32(request+4,height);
    for (offset=0;offset<luma+chroma*2u;++offset) request[8u+offset]=planes[offset];
    result = gamepkg_port_call(GAMEPKG_PORT_THP_DISPLAY,request,request_bytes,rgba,needed);
    return result < 0 ? result : ((uint32_t)result==needed ? (int32_t)needed : GAMEPKG_PORT_FAILED);
}
/* Fixed-width, directly compiled SIMD helpers. Inputs/outputs are ordinary
 * fixed-width arrays, never native handles or wire structs. Reductions use a
 * fixed lane order so every reviewed backend returns the same value. */
static inline void gamepkg_simd_u32x4_add(const uint32_t a[4],const uint32_t b[4],uint32_t out[4]) {
#if defined(GAMEPKG_SIMD_X86_SSE2)
    __m128i av=_mm_loadu_si128((const __m128i *)a);
    __m128i bv=_mm_loadu_si128((const __m128i *)b);
    _mm_storeu_si128((__m128i *)out,_mm_add_epi32(av,bv));
#elif defined(GAMEPKG_SIMD_AARCH64_NEON)
    uint32x4_t value=vaddq_u32(vld1q_u32(a),vld1q_u32(b));
    vst1q_u32(out,value);
#else
    unsigned i;
    for (i=0;i<4;i++) out[i]=a[i]+b[i];
#endif
}
static inline void gamepkg_simd_u32x8_add(const uint32_t a[8],const uint32_t b[8],uint32_t out[8]) {
#if defined(GAMEPKG_SIMD_X86_AVX2)
    __m256i av=_mm256_loadu_si256((const __m256i *)a);
    __m256i bv=_mm256_loadu_si256((const __m256i *)b);
    _mm256_storeu_si256((__m256i *)out,_mm256_add_epi32(av,bv));
#else
    gamepkg_simd_u32x4_add(a,b,out);
    gamepkg_simd_u32x4_add(a+4,b+4,out+4);
#endif
}
static inline void gamepkg_simd_f32x4_add(const float a[4],const float b[4],float out[4]) {
#if defined(GAMEPKG_SIMD_X86_SSE2)
    __m128 av=_mm_loadu_ps(a);
    __m128 bv=_mm_loadu_ps(b);
    _mm_storeu_ps(out,_mm_add_ps(av,bv));
#elif defined(GAMEPKG_SIMD_AARCH64_NEON)
    float32x4_t value=vaddq_f32(vld1q_f32(a),vld1q_f32(b));
    vst1q_f32(out,value);
#else
    unsigned i;
    for (i=0;i<4;i++) out[i]=a[i]+b[i];
#endif
}
static inline void gamepkg_simd_f32x4_mul(const float a[4],const float b[4],float out[4]) {
#if defined(GAMEPKG_SIMD_X86_SSE2)
    __m128 av=_mm_loadu_ps(a);
    __m128 bv=_mm_loadu_ps(b);
    _mm_storeu_ps(out,_mm_mul_ps(av,bv));
#elif defined(GAMEPKG_SIMD_AARCH64_NEON)
    float32x4_t value=vmulq_f32(vld1q_f32(a),vld1q_f32(b));
    vst1q_f32(out,value);
#else
    unsigned i;
    for (i=0;i<4;i++) out[i]=a[i]*b[i];
#endif
}
static inline void gamepkg_simd_f32x8_add(const float a[8],const float b[8],float out[8]) {
#if defined(GAMEPKG_SIMD_X86_AVX2)
    __m256 av=_mm256_loadu_ps(a);
    __m256 bv=_mm256_loadu_ps(b);
    _mm256_storeu_ps(out,_mm256_add_ps(av,bv));
#else
    gamepkg_simd_f32x4_add(a,b,out);
    gamepkg_simd_f32x4_add(a+4,b+4,out+4);
#endif
}
static inline void gamepkg_simd_f32x8_mul(const float a[8],const float b[8],float out[8]) {
#if defined(GAMEPKG_SIMD_X86_AVX2)
    __m256 av=_mm256_loadu_ps(a);
    __m256 bv=_mm256_loadu_ps(b);
    _mm256_storeu_ps(out,_mm256_mul_ps(av,bv));
#else
    gamepkg_simd_f32x4_mul(a,b,out);
    gamepkg_simd_f32x4_mul(a+4,b+4,out+4);
#endif
}
static inline uint32_t gamepkg_simd_u32x8_wrapping_sum(const uint32_t v[8]) {
    uint32_t total=0;
    unsigned i;
    for (i=0;i<8;i++) total+=v[i];
    return total;
}
static inline float gamepkg_simd_f32x4_sum(const float v[4]) {
    return ((v[0]+v[1])+v[2])+v[3];
}
static inline float gamepkg_simd_f32x8_sum(const float v[8]) {
    float total=v[0];
    unsigned i;
    for (i=1;i<8;i++) total+=v[i];
    return total;
}
static inline void gamepkg_simd_f32x4_sub(const float a[4],const float b[4],float out[4]) {
#if defined(GAMEPKG_SIMD_X86_SSE2)
    __m128 av=_mm_loadu_ps(a),bv=_mm_loadu_ps(b);
    _mm_storeu_ps(out,_mm_sub_ps(av,bv));
#elif defined(GAMEPKG_SIMD_AARCH64_NEON)
    vst1q_f32(out,vsubq_f32(vld1q_f32(a),vld1q_f32(b)));
#else
    unsigned i; for(i=0;i<4;i++) out[i]=a[i]-b[i];
#endif
}
static inline void gamepkg_simd_f32x4_div(const float a[4],const float b[4],float out[4]) {
#if defined(GAMEPKG_SIMD_X86_SSE2)
    __m128 av=_mm_loadu_ps(a),bv=_mm_loadu_ps(b);
    _mm_storeu_ps(out,_mm_div_ps(av,bv));
#elif defined(GAMEPKG_SIMD_AARCH64_NEON)
    vst1q_f32(out,vdivq_f32(vld1q_f32(a),vld1q_f32(b)));
#else
    unsigned i; for(i=0;i<4;i++) out[i]=a[i]/b[i];
#endif
}
static inline void gamepkg_simd_f32x8_sub(const float a[8],const float b[8],float out[8]) {
#if defined(GAMEPKG_SIMD_X86_AVX2)
    __m256 av=_mm256_loadu_ps(a),bv=_mm256_loadu_ps(b);
    _mm256_storeu_ps(out,_mm256_sub_ps(av,bv));
#else
    gamepkg_simd_f32x4_sub(a,b,out); gamepkg_simd_f32x4_sub(a+4,b+4,out+4);
#endif
}
static inline void gamepkg_simd_f32x8_div(const float a[8],const float b[8],float out[8]) {
#if defined(GAMEPKG_SIMD_X86_AVX2)
    __m256 av=_mm256_loadu_ps(a),bv=_mm256_loadu_ps(b);
    _mm256_storeu_ps(out,_mm256_div_ps(av,bv));
#else
    gamepkg_simd_f32x4_div(a,b,out); gamepkg_simd_f32x4_div(a+4,b+4,out+4);
#endif
}
static inline void gamepkg_simd_u32x4_sub(const uint32_t a[4],const uint32_t b[4],uint32_t out[4]) {
#if defined(GAMEPKG_SIMD_X86_SSE2)
    __m128i av=_mm_loadu_si128((const __m128i *)a),bv=_mm_loadu_si128((const __m128i *)b);
    _mm_storeu_si128((__m128i *)out,_mm_sub_epi32(av,bv));
#elif defined(GAMEPKG_SIMD_AARCH64_NEON)
    vst1q_u32(out,vsubq_u32(vld1q_u32(a),vld1q_u32(b)));
#else
    unsigned i; for(i=0;i<4;i++) out[i]=a[i]-b[i];
#endif
}
static inline void gamepkg_simd_u32x4_and(const uint32_t a[4],const uint32_t b[4],uint32_t out[4]) {
#if defined(GAMEPKG_SIMD_X86_SSE2)
    __m128i av=_mm_loadu_si128((const __m128i *)a),bv=_mm_loadu_si128((const __m128i *)b);
    _mm_storeu_si128((__m128i *)out,_mm_and_si128(av,bv));
#elif defined(GAMEPKG_SIMD_AARCH64_NEON)
    vst1q_u32(out,vandq_u32(vld1q_u32(a),vld1q_u32(b)));
#else
    unsigned i; for(i=0;i<4;i++) out[i]=a[i]&b[i];
#endif
}
static inline void gamepkg_simd_u32x4_or(const uint32_t a[4],const uint32_t b[4],uint32_t out[4]) {
#if defined(GAMEPKG_SIMD_X86_SSE2)
    __m128i av=_mm_loadu_si128((const __m128i *)a),bv=_mm_loadu_si128((const __m128i *)b);
    _mm_storeu_si128((__m128i *)out,_mm_or_si128(av,bv));
#elif defined(GAMEPKG_SIMD_AARCH64_NEON)
    vst1q_u32(out,vorrq_u32(vld1q_u32(a),vld1q_u32(b)));
#else
    unsigned i; for(i=0;i<4;i++) out[i]=a[i]|b[i];
#endif
}
static inline void gamepkg_simd_u32x4_xor(const uint32_t a[4],const uint32_t b[4],uint32_t out[4]) {
#if defined(GAMEPKG_SIMD_X86_SSE2)
    __m128i av=_mm_loadu_si128((const __m128i *)a),bv=_mm_loadu_si128((const __m128i *)b);
    _mm_storeu_si128((__m128i *)out,_mm_xor_si128(av,bv));
#elif defined(GAMEPKG_SIMD_AARCH64_NEON)
    vst1q_u32(out,veorq_u32(vld1q_u32(a),vld1q_u32(b)));
#else
    unsigned i; for(i=0;i<4;i++) out[i]=a[i]^b[i];
#endif
}
static inline void gamepkg_simd_u32x8_sub(const uint32_t a[8],const uint32_t b[8],uint32_t out[8]) {
#if defined(GAMEPKG_SIMD_X86_AVX2)
    __m256i av=_mm256_loadu_si256((const __m256i *)a),bv=_mm256_loadu_si256((const __m256i *)b);
    _mm256_storeu_si256((__m256i *)out,_mm256_sub_epi32(av,bv));
#else
    gamepkg_simd_u32x4_sub(a,b,out); gamepkg_simd_u32x4_sub(a+4,b+4,out+4);
#endif
}
static inline void gamepkg_simd_u32x8_and(const uint32_t a[8],const uint32_t b[8],uint32_t out[8]) {
#if defined(GAMEPKG_SIMD_X86_AVX2)
    __m256i av=_mm256_loadu_si256((const __m256i *)a),bv=_mm256_loadu_si256((const __m256i *)b);
    _mm256_storeu_si256((__m256i *)out,_mm256_and_si256(av,bv));
#else
    gamepkg_simd_u32x4_and(a,b,out); gamepkg_simd_u32x4_and(a+4,b+4,out+4);
#endif
}
static inline void gamepkg_simd_u32x8_or(const uint32_t a[8],const uint32_t b[8],uint32_t out[8]) {
#if defined(GAMEPKG_SIMD_X86_AVX2)
    __m256i av=_mm256_loadu_si256((const __m256i *)a),bv=_mm256_loadu_si256((const __m256i *)b);
    _mm256_storeu_si256((__m256i *)out,_mm256_or_si256(av,bv));
#else
    gamepkg_simd_u32x4_or(a,b,out); gamepkg_simd_u32x4_or(a+4,b+4,out+4);
#endif
}
static inline void gamepkg_simd_u32x8_xor(const uint32_t a[8],const uint32_t b[8],uint32_t out[8]) {
#if defined(GAMEPKG_SIMD_X86_AVX2)
    __m256i av=_mm256_loadu_si256((const __m256i *)a),bv=_mm256_loadu_si256((const __m256i *)b);
    _mm256_storeu_si256((__m256i *)out,_mm256_xor_si256(av,bv));
#else
    gamepkg_simd_u32x4_xor(a,b,out); gamepkg_simd_u32x4_xor(a+4,b+4,out+4);
#endif
}
static inline void gamepkg_simd_u32x4_shl(const uint32_t v[4],uint32_t bits,uint32_t out[4]) {
    unsigned i; bits&=31u; for(i=0;i<4;i++) out[i]=v[i]<<bits;
}
static inline void gamepkg_simd_u32x4_shr(const uint32_t v[4],uint32_t bits,uint32_t out[4]) {
    unsigned i; bits&=31u; for(i=0;i<4;i++) out[i]=v[i]>>bits;
}
static inline void gamepkg_simd_u32x8_shl(const uint32_t v[8],uint32_t bits,uint32_t out[8]) {
    unsigned i; bits&=31u; for(i=0;i<8;i++) out[i]=v[i]<<bits;
}
static inline void gamepkg_simd_u32x8_shr(const uint32_t v[8],uint32_t bits,uint32_t out[8]) {
    unsigned i; bits&=31u; for(i=0;i<8;i++) out[i]=v[i]>>bits;
}
static inline void gamepkg_simd_f32x4_abs(const float v[4],float out[4]) {
    unsigned i; for(i=0;i<4;i++) out[i]=v[i]<0.0f?-v[i]:v[i];
}
static inline void gamepkg_simd_f32x8_abs(const float v[8],float out[8]) {
    unsigned i; for(i=0;i<8;i++) out[i]=v[i]<0.0f?-v[i]:v[i];
}
static inline void gamepkg_simd_f32x4_neg(const float v[4],float out[4]) {
    unsigned i; for(i=0;i<4;i++) out[i]=-v[i];
}
static inline void gamepkg_simd_f32x8_neg(const float v[8],float out[8]) {
    unsigned i; for(i=0;i<8;i++) out[i]=-v[i];
}
static inline void gamepkg_simd_f32x4_mul_add(const float a[4],const float b[4],const float c[4],float out[4]) {
    float ab[4]; gamepkg_simd_f32x4_mul(a,b,ab); gamepkg_simd_f32x4_add(ab,c,out);
}
static inline void gamepkg_simd_f32x8_mul_add(const float a[8],const float b[8],const float c[8],float out[8]) {
    float ab[8]; gamepkg_simd_f32x8_mul(a,b,ab); gamepkg_simd_f32x8_add(ab,c,out);
}
static inline uint32_t gamepkg_simd_u32x4_wrapping_sum(const uint32_t v[4]) {
    return v[0]+v[1]+v[2]+v[3];
}
static inline float gamepkg_simd_f32x4_product(const float v[4]) {
    return ((v[0]*v[1])*v[2])*v[3];
}
static inline float gamepkg_simd_f32x8_product(const float v[8]) {
    float total=v[0]; unsigned i; for(i=1;i<8;i++) total*=v[i]; return total;
}
static inline float gamepkg_simd_f32x4_dot(const float a[4],const float b[4]) {
    float product[4]; gamepkg_simd_f32x4_mul(a,b,product); return gamepkg_simd_f32x4_sum(product);
}
static inline float gamepkg_simd_f32x8_dot(const float a[8],const float b[8]) {
    float product[8]; gamepkg_simd_f32x8_mul(a,b,product); return gamepkg_simd_f32x8_sum(product);
}
static inline float gamepkg_simd_f32x4_min(const float v[4]) {
    float total=v[0]; unsigned i;
    for(i=1;i<4;i++) {
        if(total!=total) return total;
        if(v[i]!=v[i]) return v[i];
        total=total<v[i]?total:v[i];
    }
    return total;
}
static inline float gamepkg_simd_f32x4_max(const float v[4]) {
    float total=v[0]; unsigned i;
    for(i=1;i<4;i++) {
        if(total!=total) return total;
        if(v[i]!=v[i]) return v[i];
        total=total>v[i]?total:v[i];
    }
    return total;
}
static inline float gamepkg_simd_f32x8_min(const float v[8]) {
    float total=v[0]; unsigned i;
    for(i=1;i<8;i++) {
        if(total!=total) return total;
        if(v[i]!=v[i]) return v[i];
        total=total<v[i]?total:v[i];
    }
    return total;
}
static inline float gamepkg_simd_f32x8_max(const float v[8]) {
    float total=v[0]; unsigned i;
    for(i=1;i<8;i++) {
        if(total!=total) return total;
        if(v[i]!=v[i]) return v[i];
        total=total>v[i]?total:v[i];
    }
    return total;
}
/* Zero on success; on error, *stream is unchanged. stream must be writable. */
static inline int32_t gamepkg_port_audio_stream_create_with_rate(
    uint32_t sample_rate, uint32_t layout, uint32_t frames, uint64_t *stream) {
    uint8_t request[16], reply[8];
    gamepkg_port_put_u32(request,0);
    gamepkg_port_put_u32(request+4,sample_rate);
    gamepkg_port_put_u32(request+8,layout);
    gamepkg_port_put_u32(request+12,frames);
    int32_t result=gamepkg_port_call(GAMEPKG_PORT_AUDIO_STREAM_CREATE,request,16,reply,8);
    if (result<0) return result;
    if (result!=8) return GAMEPKG_PORT_FAILED;
    *stream=gamepkg_port_get_u64(reply);
    return 0;
}
static inline int32_t gamepkg_port_audio_stream_create(
    uint32_t layout, uint32_t frames, uint64_t *stream) {
    return gamepkg_port_audio_stream_create_with_rate(48000u,layout,frames,stream);
}
static inline int32_t gamepkg_port_file_read(uint64_t file, uint64_t offset, uint8_t *out, uint32_t length) {
    uint8_t request[20];
    gamepkg_port_put_u64(request,file);gamepkg_port_put_u64(request+8,offset);gamepkg_port_put_u32(request+16,length);
    return gamepkg_port_call(GAMEPKG_PORT_FILE_READ,request,20,out,length);
}
static inline int32_t gamepkg_port_window(uint32_t command,uint32_t value) {
    uint8_t request[8];gamepkg_port_put_u32(request,command);gamepkg_port_put_u32(request+4,value);
    return gamepkg_port_call(GAMEPKG_PORT_WINDOW_COMMAND,request,command==4u?4u:8u,0,0);
}
#endif
