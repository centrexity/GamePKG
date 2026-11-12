#ifndef GAMEPKG_NATIVE_LAUNCH_ARGS_H
#define GAMEPKG_NATIVE_LAUNCH_ARGS_H
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
/* Parse fixed graphics flags only. The existing renderer/settings reader
 * remains authoritative; command-line values become its launch overrides. */
static int gamepkg_launch_option(int argc, char **argv, int *index)
{
    if (!argv || !index || *index < 1 || *index >= argc) return 0;
    const char *key = argv[*index], *value = NULL, *env = NULL;
    char sharpen[8], scale[4];
    unsigned kind = 0;
    if (!strcmp(key, "--fullscreen") || !strcmp(key, "--windowed")) {
        env = "RECOMP_PRESENT_FULLSCREEN";
        value = !strcmp(key, "--fullscreen") ? "1" : "0";
    } else {
        static const struct { const char *arg, *env; unsigned kind; } fields[] = {
            {"--window-mode", "RECOMP_PRESENT_FULLSCREEN", 1},
            {"--aspect-ratio", "RECOMP_DISPLAY_WIDESCREEN", 2},
            {"--render-scale", "RECOMP_RENDER_SCALE", 3},
            {"--antialiasing", "RECOMP_GFX_AA", 4},
            {"--texture-lod", "RECOMP_GFX_LOD_BIAS", 5},
            {"--sharpening-percent", "RECOMP_GFX_SHARPEN", 6},
            {"--widescreen-hud", "RECOMP_GFX_HUD_FIX", 7},
            {"--menu-hud-scale", "RECOMP_GFX_UI_SCALE", 8}
        };
        for (unsigned i = 0; i < sizeof fields / sizeof fields[0]; i++)
            if (!strcmp(key, fields[i].arg)) { env = fields[i].env; kind = fields[i].kind; break; }
        if (!env) return 0;
        if (*index + 1 >= argc) goto invalid;
        value = argv[++*index];
        switch (kind) {
        case 1:
            if (!strcmp(value, "fullscreen")) value = "1";
            else if (!strcmp(value, "windowed")) value = "0";
            else goto invalid;
            break;
        case 2:
            if (!strcmp(value, "16:9")) value = "1";
            else if (!strcmp(value, "4:3")) value = "0";
            else goto invalid;
            break;
        case 3: case 8:
            if (strlen(value) != 1 || value[0] < '1' || value[0] > '4') goto invalid;
            if (kind == 8) {
                if (value[0] == '1') value = "off";
                else { scale[0] = value[0]; scale[1] = 'x'; scale[2] = 0; value = scale; }
            }
            break;
        case 4:
            if (strcmp(value,"off") && strcmp(value,"fxaa") && strcmp(value,"smaa")) goto invalid;
            break;
        case 5:
            if (!strcmp(value,"off")) value = "0";
            else if (strcmp(value,"auto") && strcmp(value,"-0.5") && strcmp(value,"-1") && strcmp(value,"-1.5")) goto invalid;
            break;
        case 6: {
            size_t length = strlen(value); unsigned percent = 0;
            if (!length || length > 3) goto invalid;
            for (size_t i = 0; i < length; i++) {
                if (value[i] < '0' || value[i] > '9') goto invalid;
                percent = percent * 10u + (unsigned)(value[i] - '0');
            }
            if (percent > 100) goto invalid;
            snprintf(sharpen,sizeof sharpen,"%u.%02u",percent / 100u,percent % 100u);value = sharpen;
            break;
        }
        case 7:
            if (!strcmp(value,"true")) value = "1";
            else if (!strcmp(value,"false")) value = "0";
            else goto invalid;
            break;
        }
    }
#if defined(_WIN32)
    if (_putenv_s(env, value)) goto invalid;
#else
    if (setenv(env, value, 1)) goto invalid;
#endif
    return 1;
invalid:
    fprintf(stderr,"Invalid or missing value for %s\n",key);
    return -1;
}
#endif
