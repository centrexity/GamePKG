#ifndef HALO_REF_H
#define HALO_REF_H
/*
 * Pointer-width references for persisted and shared engine data.
 *
 * A halo_addr is 4 bytes on every ABI. Zero is null. Any other value is a
 * guest address: either an absolute address in a registered arena (the
 * numbers stored in Xbox maps and save images) or an offset when the
 * arena's origin is 0. The host pointer is arena->base + (addr - origin).
 * Disc bytes are not rewritten. Callers resolve on access and store with
 * halo_addr_from_host, which writes the guest address back.
 *
 * halo_datum matches the engine's datum index: low 16 bits are the slot,
 * high 16 bits are the salt. It is not a pointer.
 */
#include <stddef.h>
#include <stdint.h>

#ifndef HALO_LAYOUT_H
typedef uint32_t halo_addr;
#endif
typedef uint32_t halo_datum;

typedef struct halo_arena {
	uint8_t *base;
	uint32_t origin;
	uint32_t size;
} halo_arena;

static inline int halo_addr_is_null(halo_addr addr)
{
	return addr == 0;
}

static inline void *halo_addr_resolve(const halo_arena *arena, halo_addr addr)
{
	uint32_t offset;

	if (!arena || !arena->base || addr == 0 || addr < arena->origin)
		return NULL;
	offset = addr - arena->origin;
	if (offset >= arena->size)
		return NULL;
	return arena->base + offset;
}

static inline halo_addr halo_addr_from_host(const halo_arena *arena, const void *pointer)
{
	uintptr_t base;
	uintptr_t value;
	uint32_t offset;

	if (!arena || !arena->base || !pointer)
		return 0;
	base = (uintptr_t)arena->base;
	value = (uintptr_t)pointer;
	if (value < base)
		return 0;
	offset = (uint32_t)(value - base);
	if ((uintptr_t)offset != value - base || offset >= arena->size)
		return 0;
	if (offset > UINT32_MAX - arena->origin)
		return 0;
	return arena->origin + offset;
}

#define HALO_RESOLVE(type, arena, addr) ((type *)halo_addr_resolve((arena), (addr)))

static inline uint16_t halo_datum_index(halo_datum datum)
{
	return (uint16_t)datum;
}

static inline uint16_t halo_datum_salt(halo_datum datum)
{
	return (uint16_t)(datum >> 16);
}

static inline halo_datum halo_datum_new(uint16_t index, uint16_t salt)
{
	return (halo_datum)index | ((halo_datum)salt << 16);
}

/* On-disk layouts. Sizes are the Xbox sizes. Both ABIs keep them. */

typedef struct halo_disk_tag_instance {
	int32_t group_tag;
	int32_t parent_group_tags[2];
	int32_t tag_index;
	halo_addr name;
	halo_addr base_address;
	uint32_t unused[2];
} halo_disk_tag_instance;

typedef struct halo_disk_tag_header {
	halo_addr tag_instances;
	int32_t scenario_tag_index;
	uint32_t checksum;
	int32_t tag_count;
	int32_t vertex_buffer_count;
	halo_addr vertex_buffers;
	int32_t index_buffer_count;
	halo_addr index_buffers;
	uint32_t signature;
} halo_disk_tag_header;

typedef struct halo_disk_tag_block {
	int32_t count;
	halo_addr address;
	halo_addr definition;
} halo_disk_tag_block;

typedef struct halo_disk_tag_reference {
	uint32_t group_tag;
	halo_addr name;
	int32_t name_length;
	int32_t index;
} halo_disk_tag_reference;

typedef struct halo_disk_tag_data {
	int32_t size;
	uint32_t pad;
	int32_t file_offset;
	halo_addr address;
	halo_addr definition;
} halo_disk_tag_data;

typedef struct halo_disk_tag_reference_definition {
	uint32_t flags;
	uint32_t group_tag;
	halo_addr group_tags;
} halo_disk_tag_reference_definition;

typedef struct halo_disk_data_iterator {
	halo_addr data;
	int16_t absolute_index;
	int16_t pad;
	int32_t datum_index;
	uint32_t signature;
} halo_disk_data_iterator;

typedef struct halo_disk_saved_pointer {
	halo_addr address;
	int32_t size;
} halo_disk_saved_pointer;

#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
_Static_assert(sizeof(halo_addr) == 4, "halo_addr is 4 bytes");
_Static_assert(sizeof(halo_datum) == 4, "halo_datum is 4 bytes");
_Static_assert(sizeof(halo_disk_tag_instance) == 0x20, "tag instance disk size");
_Static_assert(offsetof(halo_disk_tag_header, tag_count) == 0xC, "tag count offset");
_Static_assert(sizeof(halo_disk_tag_header) == 0x24, "tag header disk size");
_Static_assert(sizeof(halo_disk_tag_block) == 0xC, "tag block disk size");
_Static_assert(sizeof(halo_disk_tag_reference) == 0x10, "tag reference disk size");
_Static_assert(sizeof(halo_disk_tag_data) == 0x14, "tag data disk size");
_Static_assert(sizeof(halo_disk_tag_reference_definition) == 0xC, "tag reference definition disk size");
_Static_assert(sizeof(halo_disk_data_iterator) == 0x10, "data iterator disk size");
_Static_assert(sizeof(halo_disk_saved_pointer) == 0x8, "saved pointer disk size");
_Static_assert(offsetof(halo_disk_tag_instance, name) == 0x10, "tag name offset");
_Static_assert(offsetof(halo_disk_tag_instance, base_address) == 0x14, "tag base offset");
#endif

#endif
