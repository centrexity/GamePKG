/* Private Linux/glibc test observer. Forward every real call unchanged.
 * Records backend requests/results, never synthesizes successful presentation.
 */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdio.h>
#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>
#include <vulkan/vulkan.h>

typedef void (*Function)(void);
static _Atomic(PFN_vkGetInstanceProcAddr) instance_proc;
static _Atomic(PFN_vkGetDeviceProcAddr) device_proc;
static _Atomic(PFN_vkCreateSwapchainKHR) create_swapchain;
static _Atomic(Function (*)(const unsigned char *)) gl_proc;
static _Atomic(Function (*)(const char *)) egl_proc;
static _Atomic(void (*)(void *, unsigned long, int)) swap_ext;
static _Atomic(int (*)(unsigned int)) swap_mesa;
static _Atomic(int (*)(int)) swap_sgi;
static _Atomic(unsigned int (*)(void *, int)) swap_egl;

static void record(const char *backend, int requested, int result) {
    const char *path = getenv("GAMEPKG_PRESENTATION_TRACE");
    if (!path) return;
    FILE *file = fopen(path, "a");
    if (file) {
        fprintf(file, "%s %d %d\n", backend, requested, result);
        fclose(file);
    }
}
static VkResult traced_swapchain(VkDevice device, const VkSwapchainCreateInfoKHR *info,
                                const VkAllocationCallbacks *allocator, VkSwapchainKHR *swapchain) {
    VkResult result = create_swapchain(device, info, allocator, swapchain);
    record("vulkan", (int)info->presentMode, (int)result);
    return result;
}
static void traced_ext(void *display, unsigned long drawable, int interval) {
    swap_ext(display, drawable, interval);
    record("glx-ext", interval, 0); /* EXT has no success return value. */
}
static int traced_mesa(unsigned int interval) {
    int result = swap_mesa(interval);
    record("glx-mesa", (int)interval, result);
    return result;
}
static int traced_sgi(int interval) {
    int result = swap_sgi(interval);
    record("glx-sgi", interval, result);
    return result;
}
static unsigned int traced_egl(void *display, int interval) {
    unsigned int result = swap_egl(display, interval);
    record("egl", interval, (int)result);
    return result;
}
static void *wrap(const char *name, void *pointer);
static PFN_vkVoidFunction traced_instance(VkInstance instance, const char *name) {
    return (PFN_vkVoidFunction)wrap(name, (void *)instance_proc(instance, name));
}
static PFN_vkVoidFunction traced_device(VkDevice device, const char *name) {
    return (PFN_vkVoidFunction)wrap(name, (void *)device_proc(device, name));
}
static Function traced_gl(const unsigned char *name) {
    return (Function)wrap((const char *)name, (void *)gl_proc(name));
}
static Function traced_egl_proc(const char *name) {
    return (Function)wrap(name, (void *)egl_proc(name));
}
static void *wrap(const char *name, void *pointer) {
    if (!pointer) return NULL;
#define WRAP(symbol, storage, type, wrapper) \
    if (!strcmp(name, symbol)) { if (pointer == (void *)wrapper) return pointer; storage = (type)pointer; return (void *)wrapper; }
    WRAP("vkGetInstanceProcAddr", instance_proc, PFN_vkGetInstanceProcAddr, traced_instance)
    WRAP("vkGetDeviceProcAddr", device_proc, PFN_vkGetDeviceProcAddr, traced_device)
    WRAP("vkCreateSwapchainKHR", create_swapchain, PFN_vkCreateSwapchainKHR, traced_swapchain)
    WRAP("glXGetProcAddress", gl_proc, Function (*)(const unsigned char *), traced_gl)
    WRAP("glXGetProcAddressARB", gl_proc, Function (*)(const unsigned char *), traced_gl)
    WRAP("eglGetProcAddress", egl_proc, Function (*)(const char *), traced_egl_proc)
    WRAP("glXSwapIntervalEXT", swap_ext, void (*)(void *, unsigned long, int), traced_ext)
    WRAP("glXSwapIntervalMESA", swap_mesa, int (*)(unsigned int), traced_mesa)
    WRAP("glXSwapIntervalSGI", swap_sgi, int (*)(int), traced_sgi)
    WRAP("eglSwapInterval", swap_egl, unsigned int (*)(void *, int), traced_egl)
#undef WRAP
    return pointer;
}
void *dlsym(void *handle, const char *name) {
    void *(*real_dlsym)(void *, const char *) =
        (void *(*)(void *, const char *))dlvsym(RTLD_NEXT, "dlsym", "GLIBC_2.2.5");
    if (!real_dlsym) abort();
    return wrap(name, real_dlsym(handle, name));
}
