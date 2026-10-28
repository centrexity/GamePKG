/* Test-only private Vulkan loader/driver diagnostics; never a player tool. */
#define WIN32_LEAN_AND_MEAN
#define VK_USE_PLATFORM_WIN32_KHR
#include <windows.h>
#include <vulkan/vulkan.h>
#include <stdio.h>
#include <string.h>
int main(void) {
  HANDLE token = NULL;
  TOKEN_ELEVATION elevation = {0}; DWORD size = 0;
  if (!OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &token)) return 1;
  if (!GetTokenInformation(token, TokenElevation, &elevation, sizeof(elevation), &size)) return 2;
  CloseHandle(token);
  printf("Test process elevated: %lu\n", (unsigned long)elevation.TokenIsElevated);
  HMODULE driver = LoadLibraryW(L"vulkan_lvp.dll");
  if (!driver) { printf("Private Lavapipe load failed: %lu\n", (unsigned long)GetLastError()); return 3; }
  char path[32768];
  if (!GetModuleFileNameA(GetModuleHandleW(L"vulkan-1.dll"), path, sizeof(path))) return 4;
  printf("Vulkan loader: %s\n", path);
  uint32_t version = 0;
  VkResult result = vkEnumerateInstanceVersion(&version);
  printf("Loader version result: %d, version: %u.%u.%u\n", result, VK_API_VERSION_MAJOR(version), VK_API_VERSION_MINOR(version), VK_API_VERSION_PATCH(version));
  if (result != VK_SUCCESS) return 5;
  const char *extensions[] = {VK_KHR_SURFACE_EXTENSION_NAME, VK_KHR_WIN32_SURFACE_EXTENSION_NAME};
  VkApplicationInfo app = {.sType=VK_STRUCTURE_TYPE_APPLICATION_INFO,.pApplicationName="GamePKG private CI probe",.apiVersion=VK_API_VERSION_1_1};
  VkInstanceCreateInfo info = {.sType=VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO,.pApplicationInfo=&app,.enabledExtensionCount=2,.ppEnabledExtensionNames=extensions};
  VkInstance instance = VK_NULL_HANDLE;
  result = vkCreateInstance(&info, NULL, &instance);
  printf("Win32 surface-capable Vulkan instance result: %d\n", result);
  if (result != VK_SUCCESS) return 6;
  VkPhysicalDevice devices[64]; uint32_t count = 64;
  result = vkEnumeratePhysicalDevices(instance, &count, devices);
  printf("Physical-device enumeration result: %d, count: %u\n", result, count);
  unsigned cpu = 0;
  if (result == VK_SUCCESS) for (uint32_t i=0; i<count; i++) {
    VkPhysicalDeviceProperties properties;
    vkGetPhysicalDeviceProperties(devices[i], &properties);
    printf("Device: %s, type: %u\n", properties.deviceName, properties.deviceType);
    cpu += properties.deviceType == VK_PHYSICAL_DEVICE_TYPE_CPU && strstr(properties.deviceName,"llvmpipe") != NULL;
  }
  vkDestroyInstance(instance, NULL);
  FreeLibrary(driver);
  if (result != VK_SUCCESS || count != 1 || cpu != 1) return 7;
  puts("Verified private Lavapipe and Win32 Vulkan surface extension; no system driver changes.");
  return 0;
}
