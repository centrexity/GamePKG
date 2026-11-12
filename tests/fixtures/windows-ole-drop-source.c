/* Maintainer-only Windows OLE source for the isolated Wine package gate.
 * Arguments: absolute dropped file, GUI executable, private readiness gate.
 * The GUI and source share one Wine instance; no game media is used. */
#define COBJMACROS
#include <ole2.h>
#include <shellapi.h>
#include <shlobj.h>
#include <stdio.h>
#include <wchar.h>
#include <windows.h>
static const wchar_t *filename;
static LONG data_refs = 1, source_refs = 1;
static HRESULT WINAPI data_qi(IDataObject *p, REFIID id, void **out) {
  if (!out)
    return E_POINTER;
  *out = NULL;
  if (IsEqualIID(id, &IID_IUnknown) || IsEqualIID(id, &IID_IDataObject)) {
    *out = p;
    InterlockedIncrement(&data_refs);
    return S_OK;
  }
  return E_NOINTERFACE;
}
static ULONG WINAPI data_add(IDataObject *p) {
  return InterlockedIncrement(&data_refs);
}
static ULONG WINAPI data_release(IDataObject *p) {
  return InterlockedDecrement(&data_refs);
}
static HRESULT WINAPI get_data(IDataObject *p, FORMATETC *f, STGMEDIUM *m) {
  if (!f || !m)
    return E_POINTER;
  if (f->cfFormat != CF_HDROP || !(f->tymed & TYMED_HGLOBAL))
    return DV_E_FORMATETC;
  SIZE_T n = wcslen(filename);
  HGLOBAL h = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT,
                          sizeof(DROPFILES) + (n + 2) * sizeof(wchar_t));
  if (!h)
    return E_OUTOFMEMORY;
  DROPFILES *d = GlobalLock(h);
  if (!d) {
    GlobalFree(h);
    return E_OUTOFMEMORY;
  }
  d->pFiles = sizeof(DROPFILES);
  d->fWide = TRUE;
  memcpy((char *)d + sizeof(DROPFILES), filename, n * sizeof(wchar_t));
  GlobalUnlock(h);
  m->tymed = TYMED_HGLOBAL;
  m->hGlobal = h;
  m->pUnkForRelease = NULL;
  return S_OK;
}
static HRESULT WINAPI get_here(IDataObject *p, FORMATETC *f, STGMEDIUM *m) {
  return E_NOTIMPL;
}
static HRESULT WINAPI query_data(IDataObject *p, FORMATETC *f) {
  return f && f->cfFormat == CF_HDROP ? S_OK : DV_E_FORMATETC;
}
static HRESULT WINAPI canonical(IDataObject *p, FORMATETC *a, FORMATETC *b) {
  return E_NOTIMPL;
}
static HRESULT WINAPI set_data(IDataObject *p, FORMATETC *f, STGMEDIUM *m,
                               BOOL r) {
  return E_NOTIMPL;
}
static HRESULT WINAPI enum_fmt(IDataObject *p, DWORD d, IEnumFORMATETC **f) {
  return E_NOTIMPL;
}
static HRESULT WINAPI advise(IDataObject *p, FORMATETC *f, DWORD d,
                             IAdviseSink *s, DWORD *c) {
  return OLE_E_ADVISENOTSUPPORTED;
}
static HRESULT WINAPI unadvise(IDataObject *p, DWORD d) {
  return OLE_E_ADVISENOTSUPPORTED;
}
static HRESULT WINAPI enum_advise(IDataObject *p, IEnumSTATDATA **s) {
  return OLE_E_ADVISENOTSUPPORTED;
}
static IDataObjectVtbl data_vtbl = {
    data_qi,   data_add, data_release, get_data, get_here, query_data,
    canonical, set_data, enum_fmt,     advise,   unadvise, enum_advise};
static HRESULT WINAPI source_qi(IDropSource *p, REFIID id, void **out) {
  if (!out)
    return E_POINTER;
  *out = NULL;
  if (IsEqualIID(id, &IID_IUnknown) || IsEqualIID(id, &IID_IDropSource)) {
    *out = p;
    InterlockedIncrement(&source_refs);
    return S_OK;
  }
  return E_NOINTERFACE;
}
static ULONG WINAPI source_add(IDropSource *p) {
  return InterlockedIncrement(&source_refs);
}
static ULONG WINAPI source_release(IDropSource *p) {
  return InterlockedDecrement(&source_refs);
}
static HRESULT WINAPI continue_drag(IDropSource *p, BOOL escape, DWORD keys) {
  if (escape)
    return DRAGDROP_S_CANCEL;
  return keys & MK_LBUTTON ? S_OK : DRAGDROP_S_DROP;
}
static HRESULT WINAPI feedback(IDropSource *p, DWORD effect) {
  return DRAGDROP_S_USEDEFAULTCURSORS;
}
static IDropSourceVtbl source_vtbl = {source_qi, source_add, source_release,
                                      continue_drag, feedback};
static IDataObject data = {&data_vtbl};
static IDropSource source = {&source_vtbl};
int wmain(int argc, wchar_t **argv) {
  if (argc != 4)
    return 2;
  filename = argv[1];
  HRESULT init = OleInitialize(NULL);
  if (FAILED(init))
    return 3;
  STARTUPINFOW startup = {0};
  startup.cb = sizeof(startup);
  PROCESS_INFORMATION child = {0};
  if (!CreateProcessW(argv[2], NULL, NULL, NULL, TRUE, 0, NULL, NULL, &startup,
                      &child))
    return 5;
  CloseHandle(child.hThread);
  DWORD began = GetTickCount();
  while (GetFileAttributesW(argv[3]) == INVALID_FILE_ATTRIBUTES) {
    if (GetTickCount() - began > 60000) {
      TerminateProcess(child.hProcess, 6);
      CloseHandle(child.hProcess);
      return 6;
    }
    Sleep(50);
  }
  DWORD effect = 0;
  puts("Native OLE drag source ready");
  fflush(stdout);
  HRESULT r = DoDragDrop(&data, &source, DROPEFFECT_COPY, &effect);
  printf("DoDragDrop hr=%08lx effect=%lu\n", (unsigned long)r,
         (unsigned long)effect);
  fflush(stdout);
  DWORD waited = WaitForSingleObject(child.hProcess, 70000);
  DWORD code = 1;
  if (waited == WAIT_OBJECT_0)
    GetExitCodeProcess(child.hProcess, &code);
  else
    TerminateProcess(child.hProcess, 7);
  CloseHandle(child.hProcess);
  OleUninitialize();
  if (code != 0)
    return 7;
  return r == DRAGDROP_S_DROP && effect == DROPEFFECT_COPY ? 0 : 4;
}
