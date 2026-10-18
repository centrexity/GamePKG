// Bounded loading notifications for the reviewed engine, Apache-2.0.
#ifndef GAMEPKG_LOADING_H
#define GAMEPKG_LOADING_H
void gamepkg_loading();
struct gamepkg_load_frame
{
    bool notified;
    gamepkg_load_frame() : notified(false) {}
    void reset() { notified = false; }
    bool request(bool active)
    {
        if(!active || notified) return false;
        notified = true;
        return true;
    }
};
struct gamepkg_resource_use
{
    bool geometry, material;
    gamepkg_resource_use() : geometry(false), material(false) {}
    void reset() { geometry = material = false; }
    void prepare(bool skin = false)
    {
        if(!geometry || (skin && !material))
        {
            gamepkg_loading();
            geometry = true;
            if(skin) material = true;
        }
    }
};
#endif
