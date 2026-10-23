/* Private implementation included only by memory.c, sharing its RAM lock.
 * No guest faults, callbacks or native execution while mutating metadata. */
typedef struct VirtualPage {
  unsigned char *data;
  uint32_t protection;
} VirtualPage;
typedef struct VirtualArea {
  struct VirtualArea *next;
  uint32_t base, length, protection, external;
  VirtualPage *pages;
} VirtualArea;
struct VirtualMap {
  uint32_t base, length, limit, used, reserved;
  VirtualArea *first;
};
static int virtual_permissions(uint32_t p) {
  switch (p) {
  case 1:
    return 0;
  case 2:
  case 0x10:
  case 0x20:
    return 1;
  case 4:
  case 0x40:
    return 3;
  default:
    return -2;
  }
}
static uint64_t virtual_end(const VirtualArea *a) {
  return (uint64_t)a->base + a->length;
}
static VirtualArea *virtual_find(VirtualMap *v, uint32_t a, uint32_t n) {
  uint64_t end = (uint64_t)a + n;
  for (VirtualArea *r = v->first; r; r = r->next)
    if (!r->external && a >= r->base && a < virtual_end(r) &&
        end <= virtual_end(r))
      return r;
  return NULL;
}
static void virtual_dispose_area(VirtualArea *a) {
  if (!a)
    return;
  if (a->pages)
    for (uint32_t i = 0; i < a->length / 4096; ++i)
      free(a->pages[i].data);
  free(a->pages);
  free(a);
}
static void virtual_destroy(VirtualMap *v) {
  if (!v)
    return;
  while (v->first) {
    VirtualArea *a = v->first;
    v->first = a->next;
    virtual_dispose_area(a);
  }
  free(v);
}
static const GamePkgX86Region *
virtual_region(GamePkgX86Memory *m, uint32_t address, GamePkgX86Region *out) {
  if (!m->virtual_map)
    return NULL;
  VirtualArea *a = virtual_find(m->virtual_map, address, 1);
  if (!a)
    return NULL;
  VirtualPage *p = &a->pages[(address - a->base) / 4096];
  if (!p->data || !virtual_permissions(p->protection))
    return NULL;
  *out =
      (GamePkgX86Region){address & ~4095u, 4096,
                         (uint32_t)virtual_permissions(p->protection), p->data};
  return out;
}
static int virtual_configure(GamePkgX86Memory *m, uint32_t base,
                             uint32_t length, uint32_t limit, int excluding) {
  if (!m || bound == m ||
      atomic_load_explicit(&m->references, memory_order_acquire) != 1 ||
      m->virtual_map || !base || !length || !limit ||
      ((base | length | limit) & 4095) ||
      ((uint64_t)base + length > (UINT64_C(1) << 32)))
    return -1;
  uint64_t end = (uint64_t)base + length;
  for (size_t i = 0; !excluding && i < m->count; ++i)
    if (base < (uint64_t)m->regions[i].base + m->regions[i].length &&
        m->regions[i].base < end)
      return -1;
  VirtualMap *v = calloc(1, sizeof(*v));
  if (!v)
    return -1;
  v->base = base;
  v->length = length;
  v->limit = limit;
  if (excluding)
    for (size_t i = 0; i < m->count; ++i) {
      uint64_t lo = m->regions[i].base & ~4095u;
      uint64_t hi =
          ((uint64_t)m->regions[i].base + m->regions[i].length + 4095) &
          ~UINT64_C(4095);
      if (lo < base)
        lo = base;
      if (hi > end)
        hi = end;
      if (lo >= hi)
        continue;
      VirtualArea **slot = &v->first;
      while (*slot && virtual_end(*slot) < lo)
        slot = &(*slot)->next;
      if (*slot && (*slot)->base <= hi) {
        VirtualArea *r = *slot;
        if (r->base < lo)
          lo = r->base;
        if (virtual_end(r) > hi)
          hi = virtual_end(r);
        while (r->next && r->next->base <= hi) {
          VirtualArea *next = r->next;
          if (virtual_end(next) > hi)
            hi = virtual_end(next);
          r->next = next->next;
          free(next);
        }
        r->base = (uint32_t)lo;
        r->length = (uint32_t)(hi - lo);
      } else {
        VirtualArea *r = calloc(1, sizeof(*r));
        if (!r) {
          virtual_destroy(v);
          return -1;
        }
        r->base = (uint32_t)lo;
        r->length = (uint32_t)(hi - lo);
        r->external = 1;
        r->next = *slot;
        *slot = r;
      }
    }
  m->virtual_map = v;
  return 0;
}
int gamepkg_x86_virtual_configure(GamePkgX86Memory *m, uint32_t base,
                                  uint32_t length, uint32_t limit) {
  return virtual_configure(m, base, length, limit, 0);
}
int gamepkg_x86_virtual_configure_excluding_fixed(GamePkgX86Memory *m,
                                                  uint32_t base,
                                                  uint32_t length,
                                                  uint32_t limit) {
  return virtual_configure(m, base, length, limit, 1);
}
int gamepkg_x86_virtual_is_configured(GamePkgX86Memory *m) {
  return m && m == bound && m->virtual_map ? 0 : -1;
}
static GamePkgX86Memory *virtual_acquire(GamePkgX86Memory *m) {
  return gamepkg_x86_virtual_is_configured(m) ? NULL : acquire();
}
static int virtual_geometry(VirtualMap *v, uint32_t a, uint32_t n) {
  return n && !(a & 4095) && !(n & 4095) && a >= v->base &&
         (uint64_t)a + n <= (uint64_t)v->base + v->length;
}
static VirtualArea *virtual_area(uint32_t a, uint32_t n, uint32_t protection) {
  VirtualArea *r = calloc(1, sizeof(*r));
  if (!r)
    return NULL;
  size_t pages = n / 4096;
  if (pages > SIZE_MAX / sizeof(*r->pages)) {
    free(r);
    return NULL;
  }
  r->pages = calloc(pages, sizeof(*r->pages));
  if (!r->pages) {
    free(r);
    return NULL;
  }
  r->base = a;
  r->length = n;
  r->protection = protection;
  return r;
}
static int virtual_commit_area(VirtualMap *v, VirtualArea *r, uint32_t a,
                               uint32_t n, uint32_t protection,
                               uint32_t require_committed) {
  uint32_t first = (a - r->base) / 4096, count = n / 4096, needed = 0;
  for (uint32_t i = 0; i < count; ++i)
    needed += r->pages[first + i].data == NULL;
  if (require_committed && needed)
    return 4;
  if ((uint64_t)needed * 4096 > v->limit - v->used)
    return 2;
  VirtualPage *stage = calloc(count, sizeof(*stage));
  if (!stage)
    return 2;
  for (uint32_t i = 0; i < count; ++i) {
    stage[i] = r->pages[first + i];
    stage[i].protection = protection;
    if (!stage[i].data) {
      stage[i].data = calloc(1, 4096);
      if (!stage[i].data) {
        for (uint32_t j = 0; j < i; ++j)
          if (!r->pages[first + j].data)
            free(stage[j].data);
        free(stage);
        return 2;
      }
    }
  }
  for (uint32_t i = 0; i < count; ++i)
    r->pages[first + i] = stage[i];
  v->used += needed * 4096;
  free(stage);
  return 0;
}
GamePkgX86VirtualResult
gamepkg_x86_virtual_reserve(GamePkgX86Memory *memory, uint32_t hint, uint32_t n,
                            uint32_t maximum, uint32_t down,
                            uint32_t protection, uint32_t commit) {
  GamePkgX86Memory *m = virtual_acquire(memory);
  if (!m)
    return (GamePkgX86VirtualResult){-1, 0, 0};
  VirtualMap *v = m->virtual_map;
  int status = -1;
  uint32_t at = 0;
  if (!n || (n & 4095) || down > 1 || commit > 1 || (hint & 65535))
    goto done;
  if (virtual_permissions(protection) < 0) {
    status = -2;
    goto done;
  }
  uint64_t hi = (uint64_t)v->base + v->length;
  if (hi > (uint64_t)maximum + 1)
    hi = (uint64_t)maximum + 1;
  if (hint) {
    if (!virtual_geometry(v, hint, n) || (uint64_t)hint + n > hi) {
      status = 1;
      goto done;
    }
    at = hint;
    for (VirtualArea *r = v->first; r; r = r->next)
      if (hint < virtual_end(r) && r->base < (uint64_t)hint + n) {
        status = 1;
        goto done;
      }
  } else {
    uint64_t low = v->base, best = 0;
    for (VirtualArea *r = v->first;; r = r ? r->next : NULL) {
      uint64_t end = r && r->base < hi ? r->base : hi;
      uint64_t candidate = down ? (end >= n ? (end - n) & ~UINT64_C(65535) : 0)
                                : (low + 65535) & ~UINT64_C(65535);
      if (candidate >= low && candidate + n <= end) {
        best = candidate;
        if (!down)
          break;
      }
      if (!r || virtual_end(r) >= hi)
        break;
      low = virtual_end(r);
    }
    if (!best) {
      status = 2;
      goto done;
    }
    at = (uint32_t)best;
  }
  VirtualArea *r = virtual_area(at, n, protection);
  if (!r) {
    status = 2;
    goto done;
  }
  if (commit) {
    status = virtual_commit_area(v, r, at, n, protection, 0);
    if (status) {
      virtual_dispose_area(r);
      goto done;
    }
  }
  VirtualArea **slot = &v->first;
  while (*slot && (*slot)->base < at)
    slot = &(*slot)->next;
  r->next = *slot;
  *slot = r;
  v->reserved += n;
  status = 0;
done:
  unlock(m);
  return (GamePkgX86VirtualResult){status, status ? 0 : at, status ? 0 : n};
}
static GamePkgX86VirtualResult virtual_change(GamePkgX86Memory *memory,
                                              uint32_t a, uint32_t n,
                                              uint32_t protection,
                                              uint32_t committed) {
  GamePkgX86Memory *m = virtual_acquire(memory);
  if (!m)
    return (GamePkgX86VirtualResult){-1, 0, 0};
  VirtualMap *v = m->virtual_map;
  int status = -1;
  if (!virtual_geometry(v, a, n))
    goto done;
  if (virtual_permissions(protection) < 0) {
    status = -2;
    goto done;
  }
  VirtualArea *r = virtual_find(v, a, n);
  if (!r) {
    status = 3;
    goto done;
  }
  status = virtual_commit_area(v, r, a, n, protection, committed);
done:
  unlock(m);
  return (GamePkgX86VirtualResult){status, status ? 0 : a, status ? 0 : n};
}
GamePkgX86VirtualResult gamepkg_x86_virtual_commit(GamePkgX86Memory *m,
                                                   uint32_t a, uint32_t n,
                                                   uint32_t protection) {
  return virtual_change(m, a, n, protection, 0);
}
GamePkgX86VirtualResult gamepkg_x86_virtual_protect(GamePkgX86Memory *m,
                                                    uint32_t a, uint32_t n,
                                                    uint32_t protection) {
  return virtual_change(m, a, n, protection, 1);
}
GamePkgX86VirtualResult gamepkg_x86_virtual_free(GamePkgX86Memory *memory,
                                                 uint32_t a, uint32_t n,
                                                 uint32_t release) {
  GamePkgX86Memory *m = virtual_acquire(memory);
  if (!m)
    return (GamePkgX86VirtualResult){-1, 0, 0};
  VirtualMap *v = m->virtual_map;
  int status = -1;
  if (release > 1 || (a & 4095) || (n & 4095))
    goto done;
  VirtualArea *r = virtual_find(v, a, n ? n : 1);
  if (!r) {
    status = 3;
    goto done;
  }
  if (!n) {
    if (a != r->base) {
      status = 3;
      goto done;
    }
    n = r->length;
  }
  if (!virtual_geometry(v, a, n)) {
    status = -1;
    goto done;
  }
  VirtualArea *left = NULL, *right = NULL;
  if (release) {
    uint32_t l = a - r->base,
             q = (uint32_t)(virtual_end(r) - ((uint64_t)a + n));
    if (l) {
      left = virtual_area(r->base, l, r->protection);
      if (!left) {
        status = 2;
        goto done;
      }
    }
    if (q) {
      right = virtual_area(a + n, q, r->protection);
      if (!right) {
        virtual_dispose_area(left);
        status = 2;
        goto done;
      }
    }
    if (left)
      for (uint32_t i = 0; i < l / 4096; ++i)
        left->pages[i] = r->pages[i];
    if (right)
      for (uint32_t i = 0; i < q / 4096; ++i)
        right->pages[i] = r->pages[(a + n - r->base) / 4096 + i];
  }
  for (uint32_t i = (a - r->base) / 4096; i < (a - r->base + n) / 4096; ++i) {
    if (r->pages[i].data) {
      free(r->pages[i].data);
      r->pages[i] = (VirtualPage){0};
      v->used -= 4096;
    }
  }
  if (release) {
    VirtualArea **slot = &v->first;
    while (*slot != r)
      slot = &(*slot)->next;
    if (right) {
      right->next = r->next;
      r->next = right;
    }
    if (left) {
      left->next = r->next;
      *slot = left;
    } else
      *slot = r->next;
    free(r->pages);
    free(r);
    v->reserved -= n;
  }
  status = 0;
done:
  unlock(m);
  return (GamePkgX86VirtualResult){status, status ? 0 : a, status ? 0 : n};
}
GamePkgX86VirtualInfo gamepkg_x86_virtual_query(GamePkgX86Memory *memory,
                                                uint32_t address) {
  GamePkgX86Memory *m = virtual_acquire(memory);
  if (!m)
    return (GamePkgX86VirtualInfo){.status = -1};
  VirtualMap *v = m->virtual_map;
  GamePkgX86VirtualInfo out = {.status = -1};
  uint32_t a = address & ~4095u;
  if (a < v->base || (uint64_t)a >= (uint64_t)v->base + v->length)
    goto done;
  for (VirtualArea *p = v->first; p; p = p->next)
    if (p->external && a >= p->base && a < virtual_end(p)) {
      out.status = -2;
      goto done;
    }
  VirtualArea *r = virtual_find(v, a, 1);
  out = (GamePkgX86VirtualInfo){.base = a};
  if (!r) {
    uint64_t end = (uint64_t)v->base + v->length;
    for (VirtualArea *p = v->first; p; p = p->next)
      if (p->base > a) {
        end = p->base;
        break;
      }
    out.length = (uint32_t)(end - a);
    out.state = 0x10000;
    out.protection = 1;
    goto done;
  }
  out.allocation_base = r->base;
  out.allocation_protection = r->protection;
  out.type = 0x20000;
  uint32_t i = (a - r->base) / 4096;
  VirtualPage p = r->pages[i];
  out.state = p.data ? 0x1000 : 0x2000;
  out.protection = p.data ? p.protection : 0;
  uint32_t end = i + 1;
  while (end < r->length / 4096 &&
         ((r->pages[end].data != NULL) == (p.data != NULL)) &&
         (!p.data || r->pages[end].protection == p.protection))
    ++end;
  out.length = (end - i) * 4096;
done:
  unlock(m);
  return out;
}
int gamepkg_x86_virtual_stats(GamePkgX86Memory *memory,
                              GamePkgX86VirtualStats *out) {
  if (!out)
    return -1;
  GamePkgX86Memory *m = virtual_acquire(memory);
  if (!m)
    return -1;
  VirtualMap *v = m->virtual_map;
  *out = (GamePkgX86VirtualStats){v->reserved, v->used, v->limit};
  unlock(m);
  return 0;
}
