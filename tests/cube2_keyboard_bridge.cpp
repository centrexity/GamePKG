#include <cassert>
#include "../engines/cube2/adapters/keyboard.h"
int main()
{
    gamepkg_keyboard_state keys;
    for(unsigned key=4;key<512;key++)
    {
        assert(keys.update(key,true,false));
        assert(!keys.update(key,true,true));
        assert(!keys.update(key,false,false));assert(keys.down[key]);
        assert(keys.update(key,false,true));assert(!keys.down[key]);
        assert(keys.update(key,true,true));
        assert(!keys.update(key,true,false));
        assert(!keys.update(key,false,true));assert(keys.down[key]);
        assert(keys.update(key,true,false,true)); // Real physical repeat survives.
        assert(!keys.update(key,true,false));
        assert(keys.release(key));assert(!keys.release(key));
        assert(!keys.update(key,false,false));assert(!keys.update(key,false,true));
    }
    assert(keys.update(225,true,true));
    assert(keys.with_action_shift(0)==1);
    assert(keys.with_action_shift(2)==3);
    assert(keys.with_action_shift(0x3000)==0x3001);
    assert(!keys.update(225,true,false));
    assert(!keys.update(225,false,true));
    assert(keys.with_action_shift(1)==1); // Physical Shift still holds it.
    assert(keys.with_action_shift(0)==0); // Other physical modifier snapshots are authoritative.
    assert(keys.release(225));
    assert(keys.with_action_shift(0x3000)==0x3000);
    for(unsigned key=0;key<4;key++) assert(!keys.update(key,true,false));
    assert(!keys.update(512,true,false));assert(!keys.release(~0u));
}
