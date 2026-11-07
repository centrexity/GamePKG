/* Resolve a guest KTHREAD to a host thread handle. Included after the KE
 * shadow table in the pinned kernel bridge, also exercised by the native fixture. */
static HANDLE bridge_priority_thread(uint32_t va, int *owned)
{
    HANDLE duplicate = NULL;
    uint32_t i;
    int slot, live;
    *owned = 0;
    if (!va) return NULL;
    if (va == g_thread_obj_va ||
        (g_fs_base && va == BRIDGE_MEM32(g_fs_base + 0x28)))
        return GetCurrentThread();

    /* KE shadows also contain events/timers: only registered live threads
     * qualify. Pin the object until its shadow handle has been duplicated. */
    thobj_lock();
    slot = thobj_slot(va);
    live = slot >= 0 && s_thobj_refs[slot] > 0;
    if (live) s_thobj_refs[slot]++;
    thobj_unlock();
    if (!live) return NULL;

    ke_shadow_init();
    EnterCriticalSection(&g_ke_shadow_cs);
    for (i = 0; i < KE_SHADOW_SIZE; i++) {
        if (g_ke_shadow[i].in_use && g_ke_shadow[i].guest_va == va) {
            DuplicateHandle(GetCurrentProcess(), g_ke_shadow[i].host_handle,
                GetCurrentProcess(), &duplicate, 0, FALSE, DUPLICATE_SAME_ACCESS);
            break;
        }
    }
    LeaveCriticalSection(&g_ke_shadow_cs);
    thobj_unref(va);
    if (duplicate) *owned = 1;
    return duplicate;
}
