// Exercise the production loading state without game media or a graphics driver.
#include <cassert>
#include "../engines/cube2/adapters/loading.h"
static gamepkg_load_frame frame;
static bool running = false, closing = false;
static unsigned markers = 0;
void gamepkg_loading()
{
    if(frame.request(running && !closing)) ++markers;
}
int main()
{
    gamepkg_loading(); assert(markers == 0);
    running = true;
    gamepkg_resource_use model, shader;
    model.prepare(); shader.prepare();
    for(int i = 0; i < 10000; ++i) gamepkg_loading();
    assert(markers == 1); // Cold resources coalesce; no renewable allowance.
    frame.reset();
    model.prepare(); shader.prepare(); assert(markers == 1);
    model.prepare(true); assert(markers == 2); // First material after shadow geometry.
    frame.reset();
    for(int i = 0; i < 10000; ++i) { model.prepare(true); shader.prepare(); }
    assert(markers == 2); // Cached frames keep their ordinary deadline.
    model.reset(); shader.reset();
    model.prepare(true); shader.prepare(); assert(markers == 3);
    frame.reset(); closing = true;
    gamepkg_loading(); assert(markers == 3);
    closing = false; gamepkg_loading(); assert(markers == 4);
    frame.reset(); running = false;
    gamepkg_loading(); assert(markers == 4);
    running = true; gamepkg_loading(); assert(markers == 5);
}
