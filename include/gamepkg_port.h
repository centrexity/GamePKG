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
#define GAMEPKG_PORT_AUDIO_STREAM_CREATE 130u
#define GAMEPKG_PORT_AUDIO_STREAM_SUBMIT 131u
#define GAMEPKG_PORT_AUDIO_STREAM_STATUS 132u
#define GAMEPKG_PORT_AUDIO_STREAM_CONTROL 133u
#define GAMEPKG_PORT_AUDIO_STREAM_RELEASE 134u
#define GAMEPKG_PORT_VIEWPORT_PASS 129u
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
