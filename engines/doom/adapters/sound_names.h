/* Reviewed sound-name adaptation, including the original one-level links. */
static void gamepkg_sound_lump_name(const sfxinfo_t *sound, int prefix, char name[9]) {
  if (sound->link) sound = sound->link;
  if (prefix) snprintf(name, 9, "ds%.6s", sound->name);
  else snprintf(name, 9, "%.8s", sound->name);
}
