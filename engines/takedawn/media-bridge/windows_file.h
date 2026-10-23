#ifndef GAMEPKG_WINDOWS_DISC_FILE_H
#define GAMEPKG_WINDOWS_DISC_FILE_H
/* Host-only Windows transport for the verified, read-only disc reader.
 * This does not confine ordinary host paths; process isolation is separate.
 * The original Windows runtime integration must explicitly select these APIs.
 */
#ifndef _WIN32_WINNT
#define _WIN32_WINNT 0x0601
#endif
#include <windows.h>
#ifdef __cplusplus
extern "C" {
#endif
HANDLE WINAPI gpw_CreateFileW(LPCWSTR, DWORD, DWORD, LPSECURITY_ATTRIBUTES, DWORD, DWORD, HANDLE);
BOOL WINAPI gpw_ReadFile(HANDLE, LPVOID, DWORD, LPDWORD, LPOVERLAPPED);
BOOL WINAPI gpw_WriteFile(HANDLE, LPCVOID, DWORD, LPDWORD, LPOVERLAPPED);
BOOL WINAPI gpw_CloseHandle(HANDLE);
BOOL WINAPI gpw_SetFilePointerEx(HANDLE, LARGE_INTEGER, PLARGE_INTEGER, DWORD);
BOOL WINAPI gpw_GetFileInformationByHandle(HANDLE, LPBY_HANDLE_FILE_INFORMATION);
DWORD WINAPI gpw_GetFileAttributesW(LPCWSTR);
BOOL WINAPI gpw_GetFileAttributesExW(LPCWSTR, GET_FILEEX_INFO_LEVELS, LPVOID);
DWORD WINAPI gpw_GetFinalPathNameByHandleW(HANDLE, LPWSTR, DWORD, DWORD);
BOOL WINAPI gpw_SetEndOfFile(HANDLE);
BOOL WINAPI gpw_FlushFileBuffers(HANDLE);
HANDLE WINAPI gpw_FindFirstFileW(LPCWSTR, LPWIN32_FIND_DATAW);
BOOL WINAPI gpw_FindNextFileW(HANDLE, LPWIN32_FIND_DATAW);
BOOL WINAPI gpw_FindClose(HANDLE);
BOOL WINAPI gpw_CreateDirectoryW(LPCWSTR, LPSECURITY_ATTRIBUTES);
BOOL WINAPI gpw_DeleteFileW(LPCWSTR);
BOOL WINAPI gpw_RemoveDirectoryW(LPCWSTR);
BOOL WINAPI gpw_MoveFileExW(LPCWSTR, LPCWSTR, DWORD);
BOOL WINAPI gpw_SetFileInformationByHandle(HANDLE, FILE_INFO_BY_HANDLE_CLASS, LPVOID, DWORD);
#ifdef __cplusplus
}
#endif
#endif
