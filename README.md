# modman: A hashing mod manager that needs a better name

![CI status](https://github.com/mrkline/modman/workflows/CI/badge.svg)

Modman is a spiritual successor to tools like
[OVGME](https://wiki.hoggitworld.com/view/OVGME)
or
[JSGME](https://www.softpedia.com/get/Others/Miscellaneous/Generic-Mod-Enabler.shtml):
it copies files from (ZIP) archives or folders
into a target directory (usually the root folder of a game),
making backups of any files it overwrites.
It's designed to install mods into a game, then restore the original game files
when you decide to remove the mod.

Unlike OVGME or JSGME, modman (send me a better name and I'll mail you a pizza roll)
tracks the contents of the files it's relpacing.
This means it can notice if a game update has been applied "on top" of modded files,
and can make new backups of the updated game files and reinstall needed mod files.

## Future development

- Some sort of GUI - the plan is to have a mode that emits JSON to stdout,
  allowing a standalone GUI to invoke it and display an OVGME-like interface.

- OVGME-like network support - downloading remote repositories and checking for
  updates.

- Some sort of integrity check for the mod archives themselves. ZIP archives
  are simple (hash the .zip file), but a directory would be more complicated...
  Merkle trees?
