#include "halo_ref.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int fail(const char *message)
{
	fprintf(stderr, "halo_ref: %s\n", message);
	return 1;
}

static int test_round_trip(void)
{
	uint8_t storage[64];
	halo_arena arena;
	halo_addr addr;
	void *resolved;
	memset(storage, 0, sizeof(storage));
	memcpy(storage + 8, "ui", 2);
	arena.base = storage;
	arena.origin = 0x803A6000u;
	arena.size = sizeof(storage);
	addr = halo_addr_from_host(&arena, storage + 8);
	if (addr != 0x803A6008u)
		return fail("from_host did not keep the guest address");
	resolved = halo_addr_resolve(&arena, addr);
	if (resolved != (void *)(storage + 8))
		return fail("resolve did not return the host pointer");
	if (halo_addr_resolve(&arena, 0) != NULL)
		return fail("null resolved");
	if (halo_addr_resolve(&arena, 0x803A6000u + sizeof(storage)) != NULL)
		return fail("out of range resolved");
	if (halo_addr_from_host(&arena, storage + sizeof(storage)) != 0)
		return fail("end pointer stored");
	return 0;
}

static int test_map_fixture(void)
{
	/* A one-tag header and its name, laid out as Xbox bytes, then copied
	into a host buffer that is not at the guest address. */
	uint8_t disk[0x40];
	uint8_t *host;
	halo_arena arena;
	halo_disk_tag_header header;
	halo_disk_tag_instance *instance;
	char *name;
	memset(disk, 0, sizeof(disk));
	/* tag instance at guest 0x803A6024, name at guest 0x803A6010 ("ui"). */
	disk[0x10] = 'u';
	disk[0x11] = 'i';
	/* header.tag_instances = 0x803A6024 */
	disk[0x00] = 0x24;
	disk[0x01] = 0x60;
	disk[0x02] = 0x3A;
	disk[0x03] = 0x80;
	disk[0x0C] = 1; /* tag_count */
	memcpy(disk + 0x20, "tags", 4); /* signature at 0x20 */
	/* instance.name = 0x803A6010, instance.base_address = 0x803A6012 */
	disk[0x24 + 0x10] = 0x10;
	disk[0x24 + 0x11] = 0x60;
	disk[0x24 + 0x12] = 0x3A;
	disk[0x24 + 0x13] = 0x80;
	disk[0x24 + 0x14] = 0x12;
	disk[0x24 + 0x15] = 0x60;
	disk[0x24 + 0x16] = 0x3A;
	disk[0x24 + 0x17] = 0x80;
	host = malloc(sizeof(disk));
	if (!host)
		return fail("malloc");
	memcpy(host, disk, sizeof(disk));
	arena.base = host;
	arena.origin = 0x803A6000u;
	arena.size = sizeof(disk);
	memcpy(&header, host, sizeof(header));
	if (header.tag_count != 1 || header.signature != 0x73676174u)
		return fail("header fields");
	instance = HALO_RESOLVE(halo_disk_tag_instance, &arena, header.tag_instances);
	if (!instance)
		return fail("tag instance");
	name = HALO_RESOLVE(char, &arena, instance->name);
	if (!name || name[0] != 'u' || name[1] != 'i')
		return fail("tag name");
	if (HALO_RESOLVE(uint8_t, &arena, instance->base_address) != host + 0x12)
		return fail("tag base");
	/* Saving the host pointer writes the same guest address the disc had. */
	if (halo_addr_from_host(&arena, name) != instance->name)
		return fail("save address drifted");
	free(host);
	return 0;
}

static int test_datum(void)
{
	halo_datum datum = halo_datum_new(7, 0x1234);
	if (halo_datum_index(datum) != 7 || halo_datum_salt(datum) != 0x1234)
		return fail("datum");
	if (sizeof(datum) != 4)
		return fail("datum width");
	return 0;
}

static uint32_t lcg(uint32_t *state)
{
	*state = *state * 1664525u + 1013904223u;
	return *state;
}

static int test_bounds(void)
{
	enum { SIZE = 4096 };
	uint8_t storage[SIZE];
	uint8_t other[16];
	halo_arena arena;
	uint32_t state = 0xC0FFEEu;
	uint32_t i;

	memset(storage, 0x5a, sizeof(storage));
	arena.base = storage;
	arena.origin = 0x803A6000u;
	arena.size = SIZE;
	if (halo_addr_resolve(&arena, arena.origin) != (void *)storage)
		return fail("origin maps to base");
	if (halo_addr_from_host(&arena, storage) != arena.origin)
		return fail("base stores origin");
	if (halo_addr_resolve(&arena, arena.origin + SIZE - 1) != (void *)(storage + SIZE - 1))
		return fail("last byte");
	if (halo_addr_resolve(&arena, arena.origin + SIZE) != NULL)
		return fail("one past end");
	if (halo_addr_resolve(&arena, arena.origin - 1) != NULL)
		return fail("below origin");
	if (halo_addr_resolve(NULL, 1) != NULL)
		return fail("null arena");
	arena.base = NULL;
	if (halo_addr_resolve(&arena, 0x803A6000u) != NULL)
		return fail("null base");
	arena.base = storage;
	arena.origin = 0xFFFFFFF0u;
	arena.size = 32;
	if (halo_addr_from_host(&arena, storage + 20) != 0)
		return fail("wrap store");
	if (halo_addr_resolve(&arena, 4) != NULL)
		return fail("wrapped guest address");
	arena.origin = 0x803A6000u;
	arena.size = SIZE;
	for (i = 0; i < SIZE; i++) {
		halo_addr addr = arena.origin + i;
		if (halo_addr_resolve(&arena, addr) != (void *)(storage + i))
			return fail("in-range sweep");
		if (halo_addr_from_host(&arena, storage + i) != addr)
			return fail("store sweep");
	}
	for (i = 0; i < 10000; i++) {
		halo_addr addr = lcg(&state);
		void *resolved = halo_addr_resolve(&arena, addr);
		if (addr == 0 || addr < arena.origin || (addr - arena.origin) >= (uint32_t)SIZE) {
			if (resolved != NULL)
				return fail("fuzz accepted an outside address");
		} else if (resolved != (void *)(storage + (addr - arena.origin))) {
			return fail("fuzz missed an inside address");
		}
	}
	if (halo_addr_from_host(&arena, other) != 0)
		return fail("pointer outside the arena was stored");
	return 0;
}

int main(void)
{
	int result = test_round_trip();
	if (result)
		return result;
	result = test_map_fixture();
	if (result)
		return result;
	result = test_datum();
	if (result)
		return result;
	return test_bounds();
}
