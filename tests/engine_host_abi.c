/* Links include/gamepkg_host.h and calls every host entry point.
   Unbound calls fail closed. This checks the C calling convention and the
   ABI version, not a particular game. */
#include "gamepkg_host.h"

#include <stdio.h>
#include <stdint.h>
#include <string.h>

#if defined(_WIN32)
#include <direct.h>
#define abi_mkdir(path) _mkdir(path)
#else
#include <sys/stat.h>
#define abi_mkdir(path) mkdir((path), 0700)
#endif

static void spawn_mark(void *context)
{
	volatile int *started = context;

	if (started != NULL)
		*started = 1;
}

static int fail(const char *message)
{
	fprintf(stderr, "engine_host_abi: %s\n", message);
	return 1;
}

int main(void)
{
	uint8_t bytes[16];
	int16_t samples[4] = {1, -1, 2, -2};
	const char *line = "abi";
	int32_t result;

	if (GAMEPKG_HOST_ABI != 3)
		return fail("ABI version");
	if (sizeof(int32_t) != 4 || sizeof(uint32_t) != 4 || sizeof(uint64_t) != 8 || sizeof(float) != 4)
		return fail("scalar width");
	if (sizeof(gamepkg_path_info) != 64)
		return fail("path info size");
	gamepkg_host_begin_frame();
	if (gamepkg_frame_time_us() != 0)
		return fail("frame time");
	{
		uint64_t first = gamepkg_monotonic_time_ns();
		uint64_t second = gamepkg_monotonic_time_ns();
		uint64_t seconds = 0;
		uint32_t nanos = 0;

		if (first == 0 || second < first)
			return fail("monotonic");
		gamepkg_wall_time(NULL, NULL);
		gamepkg_wall_time(&seconds, &nanos);
		if (seconds < 1700000000ULL || nanos >= 1000000000u)
			return fail("wall");
	}
	if (gamepkg_window_state() != 0)
		return fail("window state");
	if (gamepkg_window_control(GAMEPKG_WINDOW_VISIBLE, 0) != 0)
		return fail("window control");
	if (gamepkg_effect_call(0, "missing", 0) != 0)
		return fail("effect");
	if (gamepkg_file_call(GAMEPKG_FILE_IMPORT, NULL, 0, bytes, 4) != -1)
		return fail("file");
	if (gamepkg_input_events(bytes, 16) != -1)
		return fail("input");
	(void)gamepkg_random_u64();
	if (gamepkg_network_call(0, NULL, 0, bytes, 16) != -1)
		return fail("network");
	if (gamepkg_rom_count() != 0 || gamepkg_rom_size() != -1 || gamepkg_rom_size_at(0) != -1)
		return fail("rom size");
	if (gamepkg_rom_read(0, bytes, 4) != -1 || gamepkg_rom_read_at(0, 0, bytes, 4) != -1)
		return fail("rom read");
	if (gamepkg_achievement_unlock(0) != -1)
		return fail("achievement");
	if (gamepkg_asset_read("missing", 0, bytes, 4) != -1)
		return fail("asset");
	if (gamepkg_save_size() != -1 || gamepkg_save_read(0, bytes, 4) != -1)
		return fail("save read");
	if (gamepkg_save_write(bytes, 4) != -1)
		return fail("save write");
	if (gamepkg_draw_clear(0) != -1 || gamepkg_draw_clip(0, 0, 1, 1) != -1 || gamepkg_draw_clip_reset() != -1)
		return fail("draw setup");
	if (gamepkg_draw_rect(0, 0, 1, 1, 0) != -1 || gamepkg_draw_circle(0, 0, 1, 0) != -1)
		return fail("draw shape");
	if (gamepkg_draw_image(0, 0, 1, 1, 1, 1, bytes, 4) != -1)
		return fail("image");
	if (gamepkg_draw_image_linear(0, 0, 1, 1, 1, 1, bytes, 4) != -1)
		return fail("image linear");
	if (gamepkg_audio_submit(samples, 4) != -1 || gamepkg_audio_submit_stereo(samples, 4) != -1)
		return fail("audio");
	if (gamepkg_thread_spawn(NULL, NULL, 0) != 0)
		return fail("spawn null");
	{
		static volatile int started;
		int spins;

		started = 0;
		if (gamepkg_thread_spawn(spawn_mark, (void *)&started, 0) != 1)
			return fail("spawn");
		for (spins = 0; spins < 1000000 && !started; spins++)
			gamepkg_thread_yield();
		if (!started)
			return fail("spawn did not run");
	}
	gamepkg_thread_sleep_ms(0);
	gamepkg_thread_yield();
	{
		gamepkg_path_info info;
		uint32_t handle = 0;
		uint8_t name[64];
		uint8_t body[8];
		int32_t length;
		int saw = 0;
		FILE *note;
		const char *dir = "abi-vfs";
		char file[64];

		if (gamepkg_path_stat(NULL, &info) != -1 || gamepkg_dir_open(NULL, &handle) != 0)
			return fail("vfs null");
		if (gamepkg_dir_read(0, name, sizeof(name)) != -1 || gamepkg_path_read(NULL, 0, body, 4) != -1)
			return fail("vfs bad handle");
		snprintf(file, sizeof(file), "%s/note.txt", dir);
		abi_mkdir(dir);
		note = fopen(file, "wb");
		if (!note)
			return fail("vfs fixture");
		if (fwrite("hello", 1, 5, note) != 5)
			return fail("vfs fixture");
		fclose(note);
		memset(&info, 0x5a, sizeof(info));
		if (gamepkg_path_stat("abi-vfs/missing", &info) != 0 || info.kind != GAMEPKG_PATH_NONE)
			return fail("vfs missing");
		if (gamepkg_path_stat(file, &info) != 0 || info.kind != GAMEPKG_PATH_FILE || info.size != 5)
			return fail("vfs file");
		if (gamepkg_path_stat(dir, &info) != 0 || info.kind != GAMEPKG_PATH_DIRECTORY)
			return fail("vfs dir");
		if (gamepkg_path_read(file, 1, body, 4) != 4 || memcmp(body, "ello", 4) != 0)
			return fail("vfs read");
		if (gamepkg_dir_open(file, &handle) != 0)
			return fail("vfs open file");
		if (gamepkg_dir_open(dir, &handle) != 1)
			return fail("vfs open");
		while ((length = gamepkg_dir_read(handle, name, sizeof(name))) > 0)
		{
			if (strcmp((char *)name, "note.txt") == 0)
				saw = 1;
		}
		if (length != 0 || !saw)
			return fail("vfs list");
		gamepkg_dir_close(handle);
		if (gamepkg_dir_read(handle, name, sizeof(name)) != -1)
			return fail("vfs closed");
	}
	result = gamepkg_host_log(line, 3);
	if (result != 0)
		return fail("log");
	if (gamepkg_host_log(NULL, 1) != -1)
		return fail("log null");
	puts("engine_host_abi: unbound entry points matched");
	return 0;
}
