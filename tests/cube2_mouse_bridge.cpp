#include <initializer_list>
#include <cassert>
#include "../engines/cube2/adapters/mouse.h"
int main()
{
    gamepkg_mouse_state mouse;
    for(unsigned button=1;button<=8;button++)
    {
        assert(mouse.update(button,true,false));
        assert(mouse.down[button]);
        assert(!mouse.update(button,true,false)); // Duplicate physical down.
        assert(!mouse.update(button,true,true)); // Controller also holds it.
        assert(!mouse.update(button,false,false)); // Physical up cannot release controller.
        assert(mouse.down[button]);
        assert(mouse.update(button,false,true));
        assert(!mouse.down[button]);
        assert(mouse.update(button,true,true));
        assert(!mouse.update(button,true,false));
        assert(!mouse.update(button,false,true)); // Controller up cannot release physical.
        assert(mouse.down[button]);
        assert(mouse.release(button)); // Focus reset clears both sources once.
        assert(!mouse.release(button));
        assert(!mouse.update(button,false,false));
        assert(!mouse.update(button,false,true));
        assert(mouse.update(button,true,false));
        assert(mouse.update(button,false,false)); // Complete click within one frame.
    }
    for(unsigned button=1;button<=8;button++) assert(mouse.update(button,true,false));
    for(unsigned button=1;button<=8;button++) assert(mouse.release(button));
    for(unsigned button: {0u,9u,100u,~0u}) { assert(!mouse.update(button,true,false)); assert(!mouse.release(button)); }
}
