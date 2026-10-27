/* Links include/gamepkg_host.h and calls every host entry point.
   Unbound calls fail closed. This checks the C calling convention and the
   ABI version, not a particular game. */
#include "gamepkg_host.h"

#include <stdio.h>
#include <stdint.h>

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

	if (GAMEPKG_HOST_ABI != 1)
		return fail("ABI version");
	if (sizeof(int32_t) != 4 || sizeof(uint32_t) != 4 || sizeof(uint64_t) != 8 || sizeof(float) != 4)
		return fail("scalar width");
	gamepkg_host_begin_frame();
	if (gamepkg_frame_time_us() != 0)
		return fail("frame time");
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
	result = gamepkg_host_log(line, 3);
	if (result != 0)
		return fail("log");
	if (gamepkg_host_log(NULL, 1) != -1)
		return fail("log null");
	puts("engine_host_abi: unbound entry points matched");
	return 0;
}
