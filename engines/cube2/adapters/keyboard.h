// Apache-2.0. Physical and portable-action keys share effective SDL scancodes.
#ifndef GAMEPKG_KEYBOARD_H
#define GAMEPKG_KEYBOARD_H
struct gamepkg_keyboard_state
{
    bool physical[512], actions[512], down[512];
    gamepkg_keyboard_state() { for(unsigned i=0;i<512;i++) physical[i]=actions[i]=down[i]=false; }
    bool update(unsigned key, bool pressed, bool action, bool repeat=false)
    {
        if(key<4 || key>=512) return false;
        (action ? actions[key] : physical[key])=pressed;
        bool effective=physical[key] || actions[key];
        bool changed=effective!=down[key];
        down[key]=effective;
        return changed || (pressed && repeat && !action);
    }
    // SDL 2 scancode 225 and modifier bit 1 are left Shift; the bridge asserts both.
    unsigned with_action_shift(unsigned mods) const { return mods | (actions[225] ? 1u : 0u); }
    bool release(unsigned key)
    {
        if(key<4 || key>=512) return false;
        bool wasdown=down[key];physical[key]=actions[key]=down[key]=false;
        return wasdown;
    }
};
#endif
