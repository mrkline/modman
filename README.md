# modman: A hashing mod manager that needs a better name

![CI status](https://github.com/mrkline/modman/workflows/CI/badge.svg)

Modman is a replacement for tools like
[OVGME](https://wiki.hoggitworld.com/view/OVGME)
or
[JSGME](https://www.softpedia.com/get/Others/Miscellaneous/Generic-Mod-Enabler.shtml).
It copies files from (ZIP) archives or folders
into a target directory (usually the root folder of a game),
making backups of any files it overwrites.
It's designed to easily install game mods, then restore the original game files
when you decide to remove the mods.

Unlike OVGME or JSGME, modman (send me a better name and I'll mail you a pizza roll)
tracks the contents of the files it replaces.
This can keep you out of trouble!
Consider what could happen if you install a mod and then update the game you modded.
If you're using OVGME and the update replaces modded files, your game might be
broken and hard to fix!
Your mod might not work right because the update replaced its files,
but attempting to remove the mod replaces the updated files with older versions.

Modman avoids this pickle by noticing when files don't have the contents it expects,
and includes an option to re-backup any changed files then reinstall the modded ones.
It also parallelizes most operations across all of your CPU cores,
keeping your hard drive busy while it does all this tracking.

## What can it do right now?

- Install mods from both a directory or a ZIP archive. Both are expected to be
  in an OVGME-like format, that is:

  ```
  mod.zip/
  |- README.txt (with a text description of the mod)
  |- VERSION.txt
  |- base-dir/ (the base directory of the mod)
  ```

  Unlike OVGME, `base-dir/` doesn't need to have the same name as its containing
  ZIP archive or directory.

- Uninstall mods

- List installed mods

- Check that the modded files (and backups of anything they replaced)
  contains the same stuff they did when mods were installed

- Check if installed mods files were overwritten by an update and make new
  backups accordingly (see above)

- Attempt to repair an interrupted install.

Run `modman.exe --help` for details.

## What are its future plans?

- Some sort of GUI - the plan is to have a mode that emits JSON to stdout,
  allowing a standalone GUI to invoke it and display an OVGME-like interface.

- OVGME-like network support - downloading remote repositories and checking for
  updates.

- Some sort of integrity check for the mod archives themselves. ZIP archives
  are simple (hash the .zip file), but a directory would be more complicated...
  Merkle trees?

## Technical details

Modman tracks file contents by calculating their SHA-224
(SHA-256, truncated for space) hashes. The list of installed mods,
those mods' hashes, and the files (if any) they replaced, are stored
in a JSON manifest called `modman.profile`. Backups are made to
`modman-backup/temp/`, then once complete, are atomically moved to
`modman-backup/originals/`.

## Why another tool?

As best as I can tell, OVGME hasn't been maintained for several years.
Forking it wouldn't be a very productive use of time either -
the code is a very old-school "C with classes tied directly to the Windows API"
style which would be tedious to work on and difficult to extend.
