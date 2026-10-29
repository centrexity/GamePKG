#include "gamepkg_x86_io.h"
#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_memory.h"
#include <stdlib.h>
#include <string.h>
#ifdef GAMEPKG_IO_ALLOCATOR_TEST
extern void *gamepkg_io_test_malloc(size_t);
#define malloc gamepkg_io_test_malloc
#endif
typedef struct Mounted {
  uint32_t length;
  unsigned char *prefix;
  GamePkgX86IoBackend backend;
} Mounted;
typedef struct SymbolicLink {
  struct SymbolicLink *next;
  uint32_t length, target_length;
  unsigned char bytes[];
} SymbolicLink;
struct GamePkgX86Io {
  GamePkgX86Cpu *cpu;
  GamePkgX86Memory *memory;
  GamePkgX86Handles *handles;
  size_t count;
  SymbolicLink *links;
  Mounted mounts[];
};
static unsigned lower(unsigned c) { return c >= 'A' && c <= 'Z' ? c + 32 : c; }
static int equal(const unsigned char *a, const unsigned char *b, uint32_t n,
                 uint32_t insensitive) {
  for (uint32_t i = 0; i < n; ++i)
    if ((insensitive ? lower(a[i]) : a[i]) !=
        (insensitive ? lower(b[i]) : b[i]))
      return 0;
  return 1;
}
static int valid_relative(const unsigned char *p, uint32_t n) {
  uint32_t start = 0;
  for (uint32_t i = 0; i <= n; ++i) {
    if (i < n) {
      unsigned c = p[i];
      if (c >= 128)
        return -2;
      if (c < 32 || c == 127 || c == '/' || c == ':' || c == '*' || c == '?' ||
          c == '"' || c == '<' || c == '>' || c == '|')
        return 1;
    }
    if (i == n || p[i] == '\\') {
      uint32_t size = i - start;
      if (!size && n)
        return 1;
      if ((size == 1 && p[start] == '.') ||
          (size == 2 && p[start] == '.' && p[start + 1] == '.'))
        return 1;
      start = i + 1;
    }
  }
  return 0;
}
GamePkgX86Io *gamepkg_x86_io_create(GamePkgX86Cpu *c, GamePkgX86Memory *m,
                                    GamePkgX86Handles *h,
                                    const GamePkgX86IoMount *mounts,
                                    size_t count) {
  if (!c || !gamepkg_x86_memory_is_bound(m, 0) ||
      gamepkg_x86_handles_in_domain(h, c, m) || !count || count > UINT32_MAX ||
      !mounts || count > (SIZE_MAX - sizeof(GamePkgX86Io)) / sizeof(Mounted))
    return NULL;
  size_t bytes = sizeof(GamePkgX86Io) + count * sizeof(Mounted);
  for (size_t i = 0; i < count; ++i) {
    uint32_t n = mounts[i].length;
    const unsigned char *p = mounts[i].prefix;
    if (!n || n > 65535 || !p || p[0] != '\\' || p[n - 1] == '\\' ||
        valid_relative(p + 1, n - 1) || !mounts[i].backend.context ||
        !mounts[i].backend.open || !mounts[i].backend.volume ||
        !mounts[i].backend.close || bytes > SIZE_MAX - n)
      return NULL;
    bytes += n;
    for (size_t j = 0; j < i; ++j)
      if (n == mounts[j].length && equal(p, mounts[j].prefix, n, 1))
        return NULL;
  }
  GamePkgX86Io *s = malloc(bytes);
  if (!s)
    return NULL;
  s->cpu = c;
  s->memory = m;
  s->handles = h;
  s->count = count;
  s->links = NULL;
  unsigned char *at = (unsigned char *)(s->mounts + count);
  for (size_t i = 0; i < count; ++i) {
    s->mounts[i] = (Mounted){mounts[i].length, at, mounts[i].backend};
    memcpy(at, mounts[i].prefix, mounts[i].length);
    at += mounts[i].length;
  }
  return s;
}
int gamepkg_x86_io_release(GamePkgX86Io *s) {
  if (!s || gamepkg_x86_handles_owned(s->handles, s))
    return -1;
  while (s->links) {
    SymbolicLink *next = s->links->next;
    free(s->links);
    s->links = next;
  }
  free(s);
  return 0;
}
static int owner(GamePkgX86Io *s, GamePkgX86CpuThread *t, uint32_t ip,
                 uint32_t bp) {
  return !s || gamepkg_x86_handles_in_domain(s->handles, s->cpu, s->memory) ||
                 gamepkg_x86_handles_owner(s->handles, t, ip, bp)
             ? -1
             : 0;
}
static int wrap(uint32_t a, uint32_t n) {
  return n && (uint64_t)a + n > (UINT64_C(1) << 32);
}
static void check(uint32_t a, uint32_t n, uint32_t write, uint32_t ip,
                  uint32_t bp) {
  while (n) {
    uint32_t part = n > 512 ? 512 : n;
    gamepkg_x86_memory_check(a, part, write, ip, bp);
    a += part;
    n -= part;
  }
}
static void read(uint32_t a, unsigned char *p, uint32_t n, uint32_t ip,
                 uint32_t bp) {
  while (n) {
    uint32_t part = n > 512 ? 512 : n;
    gamepkg_x86_memory_bytes(a, p, part, 0, ip, bp);
    a += part;
    p += part;
    n -= part;
  }
}
static uint32_t word(const unsigned char *p) {
  return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) |
         ((uint32_t)p[3] << 24);
}
static void store(uint32_t a, uint32_t v, uint32_t ip, uint32_t bp) {
  uint32_t w[4] = {v, 0, 0, 0};
  gamepkg_x86_memory_write(a, w, 4, 15, 0, ip, bp);
}
static void iosb(uint32_t a, uint32_t status, uint32_t info, uint32_t ip,
                 uint32_t bp) {
  uint32_t w[4] = {status, info, 0, 0};
  gamepkg_x86_memory_write(a, w, 8, 255, 0, ip, bp);
}
static GamePkgX86IoResult host(int status) {
  return (GamePkgX86IoResult){status, 0, 0, 0};
}
static GamePkgX86IoResult finish(uint32_t io, uint32_t status, uint32_t info,
                                 uint32_t handle, uint32_t ip, uint32_t bp) {
  iosb(io, status, info, ip, bp);
  return (GamePkgX86IoResult){0, status, handle, info};
}
static int file_close(void *owner, const uint32_t data[4], uint32_t terminal,
                      uint32_t *status) {
  GamePkgX86Io *s = owner;
  if (data[2] >= s->count)
    return -1;
  Mounted *m = &s->mounts[data[2]];
  return m->backend.close(m->backend.context, data[0], data[1], terminal,
                          status);
}

/* ASCII object-manager profile. Host pathname rules never interpret these
 * names. The DOS-device prefix and its drive colon are virtual syntax only.
 * Relative/codepage/other colon and trailing-target semantics stay explicit
 * unfinished features instead of guessing the host's object manager. */
static int absolute_name(const unsigned char *p, uint32_t n, int trailing) {
  if (!n)
    return 1;
  if (p[0] != '\\')
    return -2;
  uint32_t start = 1;
  int dos = n >= 3 && p[1] == '?' && p[2] == '?' && (n == 3 || p[3] == '\\');
  for (uint32_t i = 1; i <= n; ++i) {
    if (i < n) {
      unsigned c = p[i];
      if (c >= 128)
        return -2;
      if (c < 32 || c == 127 || c == '/' || c == '*' || c == '"' || c == '<' ||
          c == '>' || c == '|')
        return 1;
      if (c == '?' && !(dos && i < 3))
        return 1;
      if (c == ':' && !(dos && i == 5 && lower(p[4]) >= 'a' &&
                        lower(p[4]) <= 'z' && (n == 6 || p[6] == '\\')))
        return -2;
    }
    if (i == n || p[i] == '\\') {
      uint32_t size = i - start;
      if (!size) {
        if (i == n && n > 1 && start == n)
          return trailing ? 0 : -2;
        return 1;
      }
      if ((size == 1 && p[start] == '.') ||
          (size == 2 && p[start] == '.' && p[start + 1] == '.'))
        return 1;
      start = i + 1;
    }
  }
  return 0;
}
typedef struct NameInput {
  uint32_t buffer, length;
} NameInput;
static int name_input(uint32_t address, NameInput *out, uint32_t ip,
                      uint32_t bp) {
  if (wrap(address, 8))
    return -1;
  unsigned char raw[8];
  read(address, raw, 8, ip, bp);
  out->length = (uint32_t)raw[0] | ((uint32_t)raw[1] << 8);
  uint32_t capacity = (uint32_t)raw[2] | ((uint32_t)raw[3] << 8);
  out->buffer = word(raw + 4);
  if (out->length > capacity || wrap(out->buffer, out->length))
    return -1;
  check(out->buffer, out->length, 0, ip, bp);
  return 0;
}
static GamePkgX86IoResult named_result(int valid) {
  return valid < 0 ? host(valid) : (GamePkgX86IoResult){0, 0xc0000033u, 0, 0};
}

static int component_prefix(const unsigned char *a, uint32_t na,
                            const unsigned char *b, uint32_t nb) {
  return na <= nb && (na == nb || b[na] == '\\') && equal(a, b, na, 1);
}
/* Explicit virtual directories, with intervening directories implied by the
 * mounted device paths. Filesystem/link-parent creation stays unsupported. */
static int link_location(GamePkgX86Io *s, const unsigned char *p, uint32_t n) {
  if (n == 3 && equal(p, (const unsigned char *)"\\??", 3, 1))
    return 1;
  if (n == 7 && equal(p, (const unsigned char *)"\\Device", 7, 1))
    return 1;
  for (SymbolicLink *v = s->links; v; v = v->next) {
    if (component_prefix(p, n, v->bytes, v->length))
      return 1;
    if (component_prefix(v->bytes, v->length, p, n))
      return -2;
  }
  for (size_t i = 0; i < s->count; ++i) {
    Mounted *m = &s->mounts[i];
    if (component_prefix(p, n, m->prefix, m->length))
      return 1;
    if (component_prefix(m->prefix, m->length, p, n))
      return -2;
  }
  uint32_t parent = n;
  while (parent && p[parent - 1] != '\\')
    --parent;
  if (!parent)
    return -2;
  --parent;
  if (!parent ||
      (parent == 3 && equal(p, (const unsigned char *)"\\??", 3, 1)) ||
      (parent == 7 && equal(p, (const unsigned char *)"\\Device", 7, 1)))
    return 0;
  for (size_t i = 0; i < s->count; ++i)
    if (parent < s->mounts[i].length &&
        component_prefix(p, parent, s->mounts[i].prefix, s->mounts[i].length))
      return 0;
  return 2;
}
GamePkgX86IoResult
gamepkg_x86_io_create_symbolic_link(GamePkgX86Io *s, GamePkgX86CpuThread *t,
                                    uint32_t link, uint32_t target, uint32_t ip,
                                    uint32_t bp) {
  if (owner(s, t, ip, bp))
    return host(-1);
  NameInput a, b;
  if (name_input(link, &a, ip, bp) || name_input(target, &b, ip, bp))
    return host(-1);
  if (!a.length || !b.length)
    return (GamePkgX86IoResult){0, 0xc000000du, 0, 0};
  size_t bytes = sizeof(SymbolicLink) + (size_t)a.length + b.length;
  SymbolicLink *node = malloc(bytes);
  if (!node)
    return (GamePkgX86IoResult){0, 0xc0000017u, 0, 0};
  node->length = a.length;
  node->target_length = b.length;
  /* Both complete spans were admitted before allocation; no host callback or
   * guest mutation intervenes. No borrowed lock/guard crosses these reads. */
  read(a.buffer, node->bytes, a.length, ip, bp);
  read(b.buffer, node->bytes + a.length, b.length, ip, bp);
  int valid = absolute_name(node->bytes, a.length, 0);
  if (!valid)
    valid = absolute_name(node->bytes + a.length, b.length, 0);
  if (valid) {
    free(node);
    return named_result(valid);
  }
  int location = link_location(s, node->bytes, a.length);
  if (location) {
    free(node);
    return location < 0
               ? host(location)
               : (GamePkgX86IoResult){
                     0, location == 1 ? 0xc0000035u : 0xc000003au, 0, 0};
  }
  node->next = s->links;
  s->links = node;
  return host(0);
}
GamePkgX86IoResult
gamepkg_x86_io_delete_symbolic_link(GamePkgX86Io *s, GamePkgX86CpuThread *t,
                                    uint32_t link, uint32_t ip, uint32_t bp) {
  if (owner(s, t, ip, bp))
    return host(-1);
  NameInput a;
  if (name_input(link, &a, ip, bp))
    return host(-1);
  if (!a.length)
    return (GamePkgX86IoResult){0, 0xc000000du, 0, 0};
  unsigned char *bytes = malloc(a.length);
  if (!bytes)
    return (GamePkgX86IoResult){0, 0xc0000017u, 0, 0};
  read(a.buffer, bytes, a.length, ip, bp);
  int valid = absolute_name(bytes, a.length, 0);
  if (valid) {
    free(bytes);
    return named_result(valid);
  }
  SymbolicLink **slot = &s->links;
  while (*slot && ((*slot)->length != a.length ||
                   !equal((*slot)->bytes, bytes, a.length, 1)))
    slot = &(*slot)->next;
  free(bytes);
  if (!*slot)
    return (GamePkgX86IoResult){0, 0xc0000034u, 0, 0};
  SymbolicLink *node = *slot;
  *slot = node->next;
  free(node);
  return host(0);
}
/* One pure namespace rewrite. Resolve the first link encountered while parsing
 * components, preserve the suffix exactly, and never consult the native OS.
 *0=no link,1=rewritten,-1=allocation,-2=USHORT counted-name span overflow. */
static int rewrite_link(GamePkgX86Io *s, unsigned char **path, uint32_t *length,
                        uint32_t insensitive) {
  SymbolicLink *match = NULL;
  for (SymbolicLink *n = s->links; n; n = n->next)
    if (*length >= n->length &&
        (*length == n->length || (*path)[n->length] == '\\') &&
        equal(*path, n->bytes, n->length, insensitive) &&
        (!match || n->length < match->length))
      match = n;
  if (!match)
    return 0;
  uint32_t suffix = *length - match->length;
  if (suffix > 65535u - match->target_length)
    return -2;
  uint32_t size = match->target_length + suffix;
  unsigned char *next = malloc(size);
  if (!next)
    return -1;
  memcpy(next, match->bytes + match->length, match->target_length);
  memcpy(next + match->target_length, *path + match->length, suffix);
  free(*path);
  *path = next;
  *length = size;
  return 1;
}
/* Floyd's exact-path cycle detection: no arbitrary link-hop/execution count.
 * Repeated link objects can legitimately consume a suffix, so their identities
 * alone are insufficient to prove a cycle. All temporary host buffers are
 * released before any later guest memory access or backend callback. */
static GamePkgX86IoResult resolve_links(GamePkgX86Io *s, unsigned char **path,
                                        uint32_t *length,
                                        uint32_t insensitive) {
  if (!s->links)
    return host(0);
  unsigned char *slow = malloc(*length);
  if (!slow)
    return (GamePkgX86IoResult){0, 0xc0000017u, 0, 0};
  uint32_t slow_length = *length;
  memcpy(slow, *path, *length);
  for (;;) {
    int rc = rewrite_link(s, path, length, insensitive);
    if (rc == 1)
      rc = rewrite_link(s, path, length, insensitive);
    if (rc != 1) {
      free(slow);
      return rc == -1   ? (GamePkgX86IoResult){0, 0xc0000017u, 0, 0}
             : rc == -2 ? (GamePkgX86IoResult){0, 0xc0000106u, 0, 0}
                        : host(0);
    }
    rc = rewrite_link(s, &slow, &slow_length, insensitive);
    if (rc != 1) {
      free(slow);
      return rc == -1 ? (GamePkgX86IoResult){0, 0xc0000017u, 0, 0} : host(-1);
    }
    if (slow_length == *length && !memcmp(slow, *path, *length)) {
      free(slow);
      return host(-2);
    }
  }
}
typedef struct CreateParameters {
  uint32_t allocation, attributes, disposition;
} CreateParameters;
static GamePkgX86IoResult open_file(GamePkgX86Io *s, GamePkgX86CpuThread *t,
                                    uint32_t handle, uint32_t access,
                                    uint32_t attrs, uint32_t io, uint32_t share,
                                    uint32_t options,
                                    const CreateParameters *create, uint32_t ip,
                                    uint32_t bp) {
  if (owner(s, t, ip, bp) || wrap(handle, 4) || wrap(io, 8) || wrap(attrs, 12))
    return host(-1);
  uint32_t allocation[4] = {0};
  if (create && create->allocation) {
    if (wrap(create->allocation, 8))
      return host(-1);
    gamepkg_x86_memory_read(create->allocation, allocation, 8, ip, bp);
  }
  unsigned char raw[12];
  read(attrs, raw, 12, ip, bp);
  uint32_t root = word(raw), name = word(raw + 4), attributes = word(raw + 8),
           length = 0, buffer = 0;
  if (name) {
    if (wrap(name, 8))
      return host(-1);
    unsigned char text[8];
    read(name, text, 8, ip, bp);
    length = (uint32_t)text[0] | ((uint32_t)text[1] << 8);
    uint32_t capacity = (uint32_t)text[2] | ((uint32_t)text[3] << 8);
    buffer = word(text + 4);
    if (length > capacity)
      return host(-1);
    if (wrap(buffer, length))
      return host(-1);
    check(buffer, length, 0, ip, bp);
  }
  check(io, 8, 1, ip, bp);
  check(handle, 4, 1, ip, bp);
  if (share & ~7u)
    return finish(io, 0xc000000du, 0, 0, ip, bp);
  // Explicitly recognized families remain open rather than silently losing
  // inheritance, alerts, delete/no-buffering or other kernel semantics.
  // Backup intent never grants rights beyond the capability backend policy.
  if (attributes & ~0x40u || options & ~(0x804061u | (create ? 2u : 0u)))
    return host(-2);
  if ((options & 0x41u) == 0x41u)
    return finish(io, 0xc000000du, 0, 0, ip, bp);
  unsigned char *path = malloc(length ? length : 1);
  if (!path)
    return finish(io, 0xc0000017u, 0, 0, ip, bp);
  if (length)
    read(buffer, path, length, ip, bp);
  uint32_t mount = UINT32_MAX, parent_low = 0, parent_high = 0, offset = 0,
           insensitive = (attributes & 0x40u) != 0;
  if (length && path[0] == '\\') {
    if (root) {
      free(path);
      return host(-2);
    }
    int valid = absolute_name(path, length, 1);
    if (valid) {
      free(path);
      return valid < 0 ? host(valid) : finish(io, 0xc0000033u, 0, 0, ip, bp);
    }
    GamePkgX86IoResult resolved = resolve_links(s, &path, &length, insensitive);
    if (resolved.status || resolved.ntstatus) {
      free(path);
      return resolved.status ? resolved
                             : finish(io, resolved.ntstatus, 0, 0, ip, bp);
    }
    for (size_t i = 0; i < s->count; ++i) {
      uint32_t n = s->mounts[i].length;
      if (length >= n && (length == n || path[n] == '\\') &&
          equal(path, s->mounts[i].prefix, n, insensitive) &&
          (mount == UINT32_MAX || n > s->mounts[mount].length))
        mount = (uint32_t)i;
    }
    if (mount == UINT32_MAX) {
      free(path);
      return host(-2);
    }
    offset = s->mounts[mount].length;
    if (offset < length)
      ++offset;
  } else {
    if (!root) {
      free(path);
      return finish(io, 0xc000003bu, 0, 0, ip, bp);
    }
    GamePkgX86HandleLookup parent =
        gamepkg_x86_handles_lookup(s->handles, t, root, ip, bp);
    if (parent.status) {
      free(path);
      return host(parent.status);
    }
    if (parent.ntstatus) {
      free(path);
      return finish(io, parent.ntstatus, 0, 0, ip, bp);
    }
    if (parent.value.kind != GAMEPKG_X86_HANDLE_DIRECTORY ||
        parent.value.owner != s) {
      free(path);
      return finish(io, 0xc0000024u, 0, 0, ip, bp);
    }
    mount = parent.value.data[2];
    if (mount >= s->count) {
      free(path);
      return host(-1);
    }
    parent_low = parent.value.data[0];
    parent_high = parent.value.data[1];
  }
  uint32_t relative = length - offset;
  if (relative > 1 && path[length - 1] == '\\')
    --relative;
  int valid = valid_relative(path + offset, relative);
  if (valid) {
    free(path);
    return valid < 0 ? host(valid) : finish(io, 0xc0000033u, 0, 0, ip, bp);
  }
  GamePkgX86HandleReservation reservation =
      gamepkg_x86_handles_prepare(s->handles, t, ip, bp);
  if (reservation.status) {
    free(path);
    return reservation.status < 0 ? host(reservation.status)
                                  : finish(io, 0xc0000017u, 0, 0, ip, bp);
  }
  Mounted *m = &s->mounts[mount];
  uint32_t out[5] = {0};
  int status;
  if (create) {
    if (!m->backend.create) {
      free(path);
      if (gamepkg_x86_handles_cancel(s->handles, t, reservation.ticket))
        abort();
      return host(-2);
    }
    uint32_t request[9] = {access,
                           share,
                           options,
                           insensitive,
                           create->disposition,
                           create->attributes,
                           allocation[0],
                           allocation[1],
                           create->allocation != 0};
    status = m->backend.create(m->backend.context, parent_low, parent_high,
                               path + offset, relative, request, out);
  } else {
    status = m->backend.open(m->backend.context, parent_low, parent_high,
                             path + offset, relative, access, share, options,
                             insensitive, out);
    if (!status && !out[0])
      out[4] = 1;
  }
  free(path);
  if (status || out[0]) {
    if (gamepkg_x86_handles_cancel(s->handles, t, reservation.ticket))
      abort();
    if ((out[1] || out[2]) || (!status && (out[3] || out[4])))
      return host(-1);
    return status ? host(status) : finish(io, out[0], 0, 0, ip, bp);
  }
  if ((out[4] != 1 && out[4] != 2) || !(out[1] || out[2]) ||
      (out[3] != GAMEPKG_X86_HANDLE_DIRECTORY &&
       out[3] != GAMEPKG_X86_HANDLE_FILE)) {
    if (gamepkg_x86_handles_cancel(s->handles, t, reservation.ticket))
      abort();
    return host(-1);
  }
  GamePkgX86HandleValue value = {
      out[3], access, s, {out[1], out[2], mount, options}, file_close};
  uint32_t id;
  if (gamepkg_x86_handles_commit(s->handles, t, reservation.ticket, value, &id))
    abort();
  store(handle, id, ip, bp);
  return finish(io, 0, out[4], id, ip, bp);
}
GamePkgX86IoResult gamepkg_x86_io_open(GamePkgX86Io *s, GamePkgX86CpuThread *t,
                                       uint32_t handle, uint32_t access,
                                       uint32_t attrs, uint32_t io,
                                       uint32_t share, uint32_t options,
                                       uint32_t ip, uint32_t bp) {
  return open_file(s, t, handle, access, attrs, io, share, options, NULL, ip,
                   bp);
}
GamePkgX86IoResult gamepkg_x86_io_create_file(
    GamePkgX86Io *s, GamePkgX86CpuThread *t, uint32_t handle, uint32_t access,
    uint32_t attrs, uint32_t io, uint32_t allocation, uint32_t file_attributes,
    uint32_t share, uint32_t disposition, uint32_t options, uint32_t ip,
    uint32_t bp) {
  CreateParameters parameters = {allocation, file_attributes, disposition};
  return open_file(s, t, handle, access, attrs, io, share, options, &parameters,
                   ip, bp);
}
GamePkgX86IoResult gamepkg_x86_io_volume(GamePkgX86Io *s,
                                         GamePkgX86CpuThread *t, uint32_t id,
                                         uint32_t io, uint32_t buffer,
                                         uint32_t length, uint32_t kind,
                                         uint32_t ip, uint32_t bp) {
  if (owner(s, t, ip, bp) || wrap(io, 8) || wrap(buffer, length))
    return host(-1);
  GamePkgX86HandleLookup h =
      gamepkg_x86_handles_lookup(s->handles, t, id, ip, bp);
  if (h.status)
    return host(h.status);
  check(io, 8, 1, ip, bp);
  if (h.ntstatus)
    return finish(io, h.ntstatus, 0, 0, ip, bp);
  if (h.value.owner != s || (h.value.kind != GAMEPKG_X86_HANDLE_DIRECTORY &&
                             h.value.kind != GAMEPKG_X86_HANDLE_FILE))
    return finish(io, 0xc0000024u, 0, 0, ip, bp);
  if (kind != 3)
    return host(-2);
  if (length < 24)
    return finish(io, 0xc0000004u, 0, 0, ip, bp);
  check(buffer, 24, 1, ip, bp);
  if (h.value.data[2] >= s->count)
    return host(-1);
  Mounted *m = &s->mounts[h.value.data[2]];
  uint32_t out[7] = {0};
  int status = m->backend.volume(m->backend.context, h.value.data[0],
                                 h.value.data[1], out);
  if (status)
    return host(status);
  if (out[0])
    return finish(io, out[0], 0, 0, ip, bp);
  uint64_t total = (uint64_t)out[1] | ((uint64_t)out[2] << 32),
           available = (uint64_t)out[3] | ((uint64_t)out[4] << 32);
  if (total > INT64_MAX || available > total || !out[5] || !out[6])
    return host(-1);
  uint32_t fields[6] = {out[1], out[2], out[3], out[4], out[6], out[5]};
  unsigned char bytes[24];
  for (unsigned i = 0; i < 24; ++i)
    bytes[i] = (unsigned char)(fields[i / 4] >> (8 * (i % 4)));
  gamepkg_x86_memory_bytes(buffer, bytes, 24, 1, ip, bp);
  return finish(io, 0, 24, 0, ip, bp);
}
GamePkgX86IoResult gamepkg_x86_io_query(GamePkgX86Io *s, GamePkgX86CpuThread *t,
                                        uint32_t id, uint32_t io,
                                        uint32_t buffer, uint32_t length,
                                        uint32_t kind, uint32_t ip,
                                        uint32_t bp) {
  if (owner(s, t, ip, bp) || wrap(io, 8) || wrap(buffer, length))
    return host(-1);
  GamePkgX86HandleLookup h =
      gamepkg_x86_handles_lookup(s->handles, t, id, ip, bp);
  if (h.status)
    return host(h.status);
  check(io, 8, 1, ip, bp);
  if (h.ntstatus)
    return finish(io, h.ntstatus, 0, 0, ip, bp);
  if (h.value.owner != s || (h.value.kind != GAMEPKG_X86_HANDLE_DIRECTORY &&
                             h.value.kind != GAMEPKG_X86_HANDLE_FILE))
    return finish(io, 0xc0000024u, 0, 0, ip, bp);
  if (kind != 34)
    return host(-2);
  if (length < 56)
    return finish(io, 0xc0000004u, 0, 0, ip, bp);
  check(buffer, 56, 1, ip, bp);
  if (h.value.data[2] >= s->count)
    return host(-1);
  Mounted *m = &s->mounts[h.value.data[2]];
  if (!m->backend.query)
    return host(-2);
  uint32_t out[14] = {0};
  int status = m->backend.query(m->backend.context, h.value.data[0],
                                h.value.data[1], kind, out);
  if (status)
    return host(status);
  if (out[0])
    return finish(io, out[0], 0, 0, ip, bp);
  if ((out[10] | out[12]) & 0x80000000u)
    return host(-1);
  unsigned char bytes[52];
  for (unsigned i = 0; i < 52; ++i)
    bytes[i] = (unsigned char)(out[1 + i / 4] >> (8 * (i % 4)));
  gamepkg_x86_memory_bytes(buffer, bytes, 52, 1, ip, bp);
  return finish(io, 0, 56, 0, ip, bp);
}
typedef struct IoCopy {
  uint32_t address, length, write, ip, bp;
  unsigned char *bytes;
} IoCopy;
static _Thread_local IoCopy *active_copy;
static void copy_staged_bytes(void) {
  IoCopy *p = active_copy;
  uint32_t address = p->address, left = p->length;
  unsigned char *bytes = p->bytes;
  while (left) {
#ifdef GAMEPKG_IO_COPY_FAULT_TEST
    extern void gamepkg_io_test_copy_fault(void);
    gamepkg_io_test_copy_fault();
#endif
    uint32_t part = left > 512 ? 512 : left;
    gamepkg_x86_memory_bytes(address, bytes, part, p->write, p->ip, p->bp);
    address += part;
    bytes += part;
    left -= part;
  }
}
static int copy_staged(IoCopy *copy, GamePkgX86Fault *fault) {
  IoCopy *previous = active_copy;
  active_copy = copy;
  int rc = gamepkg_x86_run(copy_staged_bytes, fault);
  active_copy = previous;
  return rc;
}
static _Noreturn void redeliver_copy_fault(const GamePkgX86Fault *f) {
  if (f->vector == 14)
    gamepkg_x86_page_fault(f->fault_address, f->error_code, f->eip, f->ebp);
  if (f->vector == 11 || f->vector == 12 || f->vector == 13)
    gamepkg_x86_segment_fault(f->vector, f->error_code, f->eip, f->ebp);
  abort(); /* A checked RAM byte copy cannot execute other guest instructions.
            */
}
GamePkgX86IoResult
gamepkg_x86_io_transfer(GamePkgX86Io *s, GamePkgX86CpuThread *t, uint32_t write,
                        uint32_t id, uint32_t event, uint32_t apc,
                        uint32_t apc_context, uint32_t io, uint32_t address,
                        uint32_t length, uint32_t offset, uint32_t ip,
                        uint32_t bp) {
  if (owner(s, t, ip, bp) || write > 1 || wrap(io, 8) ||
      wrap(address, length) || (offset && wrap(offset, 8)))
    return host(-1);
  GamePkgX86HandleLookup h =
      gamepkg_x86_handles_lookup(s->handles, t, id, ip, bp);
  if (h.status)
    return host(h.status);
  check(io, 8, 1, ip, bp);
  if (h.ntstatus)
    return (GamePkgX86IoResult){0, h.ntstatus, 0, 0};
  if (h.value.owner != s)
    return (GamePkgX86IoResult){0, 0xc0000024u, 0, 0};
  if (h.value.kind != GAMEPKG_X86_HANDLE_FILE || event || apc || apc_context ||
      !(h.value.data[3] & 0x20u))
    return host(-2);
  uint32_t rights = h.value.access;
  if ((!write && !(rights & (1u | 0x80000000u | 0x10000000u))) ||
      (write && !(rights & (6u | 0x40000000u | 0x10000000u))))
    return (GamePkgX86IoResult){0, 0xc0000022u, 0, 0};
  uint32_t request[4] = {write, 0, 0, offset != 0};
  if (offset) {
    unsigned char raw[8];
    read(offset, raw, 8, ip, bp);
    request[1] = word(raw);
    request[2] = word(raw + 4);
  }
  check(address, length, !write, ip, bp);
  if (h.value.data[2] >= s->count)
    return host(-1);
  Mounted *m = &s->mounts[h.value.data[2]];
  if (!m->backend.stage || !m->backend.unstage || !m->backend.transfer)
    return host(-2);
  GamePkgX86IoBuffer buffer = {0};
  uint32_t status = 0;
  int rc = m->backend.stage(m->backend.context, length, &buffer, &status);
  if (rc || status) {
    if (buffer.owner || buffer.data || buffer.length) {
      if (buffer.owner)
        (void)m->backend.unstage(m->backend.context, &buffer);
      return host(-1);
    }
    return rc ? host(rc) : (GamePkgX86IoResult){0, status, 0, 0};
  }
  if (!buffer.owner || buffer.length != length ||
      (length &&
       (!buffer.data || (uintptr_t)buffer.data > UINTPTR_MAX - (length - 1)))) {
    if (buffer.owner)
      (void)m->backend.unstage(m->backend.context, &buffer);
    return host(-1);
  }
  GamePkgX86Fault fault;
  IoCopy copy = {address, length, 0, ip, bp, buffer.data};
  if (write && length) {
    rc = copy_staged(&copy, &fault);
    if (rc) {
      if (m->backend.unstage(m->backend.context, &buffer))
        return host(-1);
      if (rc == 1)
        redeliver_copy_fault(&fault);
      return host(-1);
    }
  }
  uint32_t out[3] = {0};
  rc = m->backend.transfer(m->backend.context, h.value.data[0], h.value.data[1],
                           &buffer, request, out);
  if (rc || out[1] > length || out[2] > 1 || (!out[2] && (!out[0] || out[1])) ||
      out[0] == 0x103u) {
    if (m->backend.unstage(m->backend.context, &buffer))
      return host(-1);
    return rc ? host(rc) : host(-1);
  }
  if (!write && out[1]) {
    copy.length = out[1];
    copy.write = 1;
    rc = copy_staged(&copy, &fault);
    if (rc) {
      if (m->backend.unstage(m->backend.context, &buffer))
        return host(-1);
      if (rc == 1)
        redeliver_copy_fault(&fault);
      return host(-1);
    }
  }
  if (m->backend.unstage(m->backend.context, &buffer))
    return host(-1);
  return out[2] ? finish(io, out[0], out[1], 0, ip, bp)
                : (GamePkgX86IoResult){0, out[0], 0, 0};
}
