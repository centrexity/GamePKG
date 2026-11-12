#ifndef GAMEPKG_X86_IO_H
#define GAMEPKG_X86_IO_H
#include "gamepkg_x86_handles.h"
/* Frozen reviewed HOST backend; never guest-supplied function pointers/context.
 * Callbacks receive staged host bytes/tokens only. They must preserve guest
 * GPR/FP, RAM/CPU bindings/ownership and return normally without reentry.
 * Native locks/borrowed Rust guards must be gone before return to fault-capable
 * C. Status0 supplies a valid NTSTATUS in out[0],-1 host protocol,-2 recognized
 * unfinished feature. A failed open never allocates a backend token.
 * Open out: NTSTATUS,token-low,token-high,kind (1 directory,2 file).
 * Volume out: NTSTATUS,total units low/high,available units low/high,
 * bytes per sector,sectors per allocation unit. Geometry is an explicit guest
 * profile; total/free bytes must come from actual host storage/capacity policy.
 * Close out: NTSTATUS. terminal1 means VM teardown, with no guest resume.
 * No host path/fd/pointer is exposed as a guest handle.
 */
typedef struct GamePkgX86IoBuffer {
  void *owner;
  unsigned char *data;
  uint32_t length;
} GamePkgX86IoBuffer;
typedef struct GamePkgX86IoBackend {
  void *context;
  int (*open)(void *, uint32_t parent_low, uint32_t parent_high,
              const unsigned char *, uint32_t length, uint32_t access,
              uint32_t share, uint32_t options, uint32_t insensitive,
              uint32_t out[4]);
  int (*volume)(void *, uint32_t low, uint32_t high, uint32_t out[7]);
  int (*close)(void *, uint32_t low, uint32_t high, uint32_t terminal,
               uint32_t *ntstatus);
  /* Optional create extension. Request9 logical host words: access,share,
   * options,insensitive,disposition,attributes,allocation-low,high,present.
   * Out5: NTSTATUS,token-low,high,kind,FILE_OPENED1/FILE_CREATED2.
   * Failure allocates no token; unsupported returns host-2. The caller keeps
   * request/out/context/name aligned, live and disjoint until normal return. */
  int (*create)(void *, uint32_t parent_low, uint32_t parent_high,
                const unsigned char *, uint32_t length,
                const uint32_t request[9], uint32_t out[5]);
  /* Optional information extension. Class34 out14 logical host words:
   * NTSTATUS, creation/access/write/change time low/high, allocation/EOF
   * low/high, attributes. Times are signed NT absolute100ns, sizes signed64.
   * No native structure padding or guest pointers cross this boundary. */
  int (*query)(void *, uint32_t low, uint32_t high, uint32_t information_class,
               uint32_t out[14]);
  /* Optional transfer extension, all three callbacks required. stage charges
   * the host memory budget before allocating and initializes every byte.
   * Success returns an exclusively borrowed, live host-only buffer/owner.
   * Failure allocates no owner. unstage consumes it and zeros this record;
   * every path, including a guest copy fault, must unstage before returning.
   * No buffer may outlive its backend. Record/header/data/request/out are
   * native, aligned, live and mutually disjoint; never guest-supplied pointers.
   * Transfer request4: write0/1, signed offset-low/high, present0/1.
   * Out3: NTSTATUS, processed bytes, completion0/1. completion0 preserves IOSB.
   * Full-request I/O is serialized by the backend, never per-chunk I/O. */
  int (*stage)(void *, uint32_t length, GamePkgX86IoBuffer *,
               uint32_t *ntstatus);
  int (*unstage)(void *, GamePkgX86IoBuffer *);
  int (*transfer)(void *, uint32_t low, uint32_t high, GamePkgX86IoBuffer *,
                  const uint32_t request[4], uint32_t out[3]);
} GamePkgX86IoBackend;
typedef struct GamePkgX86IoMount {
  const unsigned char *prefix;
  uint32_t length;
  GamePkgX86IoBackend backend;
} GamePkgX86IoMount;
typedef struct GamePkgX86Io GamePkgX86Io;
/* Immutable absolute NT mount prefixes, no trailing separator. Longest exact
 * component prefix wins, with OBJ_CASE_INSENSITIVE controlling ASCII folding.
 * Unknown/unconfigured absolute namespaces stop host-2; no ambient OS lookup.
 * Paths are counted Xbox ANSI strings, not native WCHAR*/
GamePkgX86Io *gamepkg_x86_io_create(GamePkgX86Cpu *, GamePkgX86Memory *,
                                    GamePkgX86Handles *,
                                    const GamePkgX86IoMount *, size_t count);
/* All handles owned by this service must be closed before quiescent release.
 * Backend contexts and CPU/RAM/table are borrowed and outlive it. */
int gamepkg_x86_io_release(GamePkgX86Io *);
typedef struct GamePkgX86IoResult {
  int status;
  uint32_t ntstatus, handle, information;
} GamePkgX86IoResult;
/* Process-local object-name links, not native filesystem symlinks. Counted
 * ANSI input spans are admitted before allocation or namespace mutation.
 * Create/delete use case-insensitive object-name collision/lookup; open honors
 * OBJ_CASE_INSENSITIVE and resolves the first link component before mounts.
 * Absolute ASCII NT names and the virtual \??\ drive syntax are supported.
 * Relative/codepage/trailing-target/other-colon semantics stay host-2; the
 * complete object-directory/handle/security lifecycle remains separate work.
 * Dangling/cyclic targets may be registered. Opens detect exact path cycles
 * without a hop quota and stop host-2; USHORT expansion overflow is
 * NAME_TOO_LONG. Deleting a link affects future name lookup, never
 * already-opened handles. No GPR/FP/IRQL/retirement or host directory/native
 * link mutation. */
GamePkgX86IoResult
gamepkg_x86_io_create_symbolic_link(GamePkgX86Io *, GamePkgX86CpuThread *,
                                    uint32_t link_ansi, uint32_t target_ansi,
                                    uint32_t eip, uint32_t ebp);
GamePkgX86IoResult gamepkg_x86_io_delete_symbolic_link(GamePkgX86Io *,
                                                       GamePkgX86CpuThread *,
                                                       uint32_t link_ansi,
                                                       uint32_t eip,
                                                       uint32_t ebp);
/* Mandatory same owner/profile/fault boundary as the shared handle table.
 * Required guest output pointers: handle4/IOSB8. Input and output spans are
 * admitted before allocation or external I/O. Outputs order: handle on success,
 * then IOSB Status/Information. Errors preserve Handle; host-2 writes nothing.
 * Staged names retain embedded-length semantics; NUL/control/forward slash,
 * colon, dot traversal and empty interior components are rejected. Non-ASCII
 * ANSI currently stops unsupported, rather than guessing a host code page.
 * Retails' complete error-precedence/attribute/option/async families remain
 * separate qualification. No success/fake directory handle for absent mounts.
 */
GamePkgX86IoResult gamepkg_x86_io_open(GamePkgX86Io *, GamePkgX86CpuThread *,
                                       uint32_t handle_pointer, uint32_t access,
                                       uint32_t attributes_pointer,
                                       uint32_t io_status_pointer,
                                       uint32_t share, uint32_t options,
                                       uint32_t eip, uint32_t ebp);
/* Xbox NtCreateFile9, sharing the same full-span admission/name/namespace
 * checks as open. A nonzero allocation pointer is read as8 explicit LE bytes
 * before any native work. WRITE_THROUGH is forwarded for create requests;
 * the backend must preserve its semantics or return explicit unsupported.
 * Supported requests/attributes depend on the configured storage provider.
 * Unfinished families remain explicit unsupported results, with no fake handle.
 * Backup intent grants no privilege beyond the configured capability policy. */
GamePkgX86IoResult gamepkg_x86_io_create_file(
    GamePkgX86Io *, GamePkgX86CpuThread *, uint32_t handle_pointer,
    uint32_t access, uint32_t attributes_pointer, uint32_t io_status_pointer,
    uint32_t allocation_size_pointer, uint32_t file_attributes, uint32_t share,
    uint32_t disposition, uint32_t options, uint32_t eip, uint32_t ebp);
/* FileFsSizeInformation3:24 explicit LE bytes, then IOSB. Short outputs fail
 * without partial payload writes. Other valid classes stop unsupported. */
GamePkgX86IoResult gamepkg_x86_io_volume(GamePkgX86Io *, GamePkgX86CpuThread *,
                                         uint32_t handle,
                                         uint32_t io_status_pointer,
                                         uint32_t buffer, uint32_t length,
                                         uint32_t information_class,
                                         uint32_t eip, uint32_t ebp);
/* NtQueryInformationFile5. Class34 requires56 output bytes, writes its52
 * defined bytes, leaves trailing padding/tail untouched, then IOSB info56.
 * All56 bytes and IOSB are admitted before native query; no native guards
 * survive guest writes. Other classes remain explicitly unsupported. */
GamePkgX86IoResult gamepkg_x86_io_query(GamePkgX86Io *, GamePkgX86CpuThread *,
                                        uint32_t handle,
                                        uint32_t io_status_pointer,
                                        uint32_t buffer, uint32_t length,
                                        uint32_t information_class,
                                        uint32_t eip, uint32_t ebp);
/* Xbox NtReadFile/NtWriteFile8. write0=read/1=write, no event/APC in the
 * synchronous profile. Optional offset is8 explicit LE bytes. Whole guest
 * spans are admitted before budgeted host staging or I/O; no transfer-size
 * quota is imposed. Data is copied through a nested fault-cleanup boundary,
 * then the allocation is consumed before any fault is redelivered. Native
 * callbacks must complete all kernel I/O before returning. Partial reads copy
 * only processed bytes; completion0 preserves IOSB, completion1 writes it last.
 * CPU/FP/IRQL/retirement state remains owned by the caller. */
GamePkgX86IoResult
gamepkg_x86_io_transfer(GamePkgX86Io *, GamePkgX86CpuThread *, uint32_t write,
                        uint32_t handle, uint32_t event, uint32_t apc,
                        uint32_t apc_context, uint32_t io_status_pointer,
                        uint32_t buffer, uint32_t length,
                        uint32_t offset_pointer, uint32_t eip, uint32_t ebp);
#endif
