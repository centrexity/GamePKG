/* Maintainer reference: actual SDL X11 button numbering under a private display. */
#include <assert.h>
#include <stdio.h>
#include <SDL.h>
#include <SDL_syswm.h>
#include <X11/extensions/XTest.h>

static void expect_button(SDL_Window *window, unsigned raw, unsigned expected, int down) {
  SDL_SysWMinfo info;
  SDL_VERSION(&info.version);
  assert(SDL_GetWindowWMInfo(window, &info));
  assert(info.subsystem == SDL_SYSWM_X11);
  assert(XTestFakeButtonEvent(info.info.x11.display, raw, down, CurrentTime));
  XFlush(info.info.x11.display);
  Uint32 started = SDL_GetTicks();
  SDL_Event event;
  while (SDL_GetTicks() - started < 2000) {
    if (!SDL_WaitEventTimeout(&event, 20)) continue;
    if (event.type != SDL_MOUSEBUTTONDOWN && event.type != SDL_MOUSEBUTTONUP) continue;
    assert(event.button.windowID == SDL_GetWindowID(window));
    assert(event.button.button == expected);
    assert(event.button.state == (down ? SDL_PRESSED : SDL_RELEASED));
    return;
  }
  fprintf(stderr, "SDL button timeout: raw=%u expected=%u down=%d mouse_focus=%d keyboard_focus=%d error=%s\n",
          raw, expected, down, SDL_GetMouseFocus()==window, SDL_GetKeyboardFocus()==window, SDL_GetError());
  assert(!"SDL did not receive the injected mouse button");
}
int main(void) {
  assert(SDL_Init(SDL_INIT_VIDEO) == 0);
  SDL_Window *window = SDL_CreateWindow("GamePKG private mouse reference", 0, 0, 200, 120, SDL_WINDOW_SHOWN);
  assert(window);
  SDL_SysWMinfo info;
  SDL_VERSION(&info.version);
  assert(SDL_GetWindowWMInfo(window, &info));
  assert(info.subsystem == SDL_SYSWM_X11);
  XWarpPointer(info.info.x11.display, None, info.info.x11.window, 0, 0, 0, 0, 25, 25);
  XSetInputFocus(info.info.x11.display, info.info.x11.window, RevertToParent, CurrentTime);
  XSync(info.info.x11.display, False);
  SDL_PumpEvents();
  /* SDL intentionally ignores clicks during its initial 10 ms focus guard.
   * Establish a settled window before testing physical button numbering. */
  SDL_Delay(20);
  SDL_PumpEvents();
  SDL_FlushEvents(SDL_FIRSTEVENT, SDL_LASTEVENT);
  for (unsigned raw = 8; raw <= 10; raw++) {
    expect_button(window, raw, raw - 4, 1);
    expect_button(window, raw, raw - 4, 0);
    printf("X11 button %u produces SDL button %u on press and release.\n", raw, raw - 4);
    fflush(stdout);
  }
  SDL_DestroyWindow(window);
  SDL_Quit();
  return 0;
}
