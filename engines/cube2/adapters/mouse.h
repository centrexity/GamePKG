// Reviewed mouse state: physical and controller sources share effective buttons.
#ifndef GAMEPKG_MOUSE_H
#define GAMEPKG_MOUSE_H
struct gamepkg_mouse_state
{
    bool physical[9], actions[9], down[9];
    gamepkg_mouse_state() { for(unsigned i=0;i<9;i++) physical[i]=actions[i]=down[i]=false; }
    bool update(unsigned button, bool pressed, bool action)
    {
        if(button<1 || button>8) return false;
        (action ? actions[button] : physical[button]) = pressed;
        bool effective = physical[button] || actions[button];
        if(effective == down[button]) return false;
        down[button] = effective;
        return true;
    }
    bool release(unsigned button)
    {
        if(button<1 || button>8) return false;
        bool wasdown=down[button];
        physical[button]=actions[button]=down[button]=false;
        return wasdown;
    }
};
#endif
