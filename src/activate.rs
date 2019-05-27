use std::collections::*;
use std::fs::*;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use failure::*;
use log::*;

use crate::file_utils::*;
use crate::journal::*;
use crate::modification::*;
use crate::profile::*;
use crate::usage::*;

static USAGE: &str = r#"Usage: modman activate [options] <MOD>

Activate a mod at the path <MOD>.
Mods can be in two formats:
  1. A directory containing a VERSION.txt file, a README.txt file,
     and a single directory, which will be treated as the root of the mod files.
  2. A .zip archive containing the same.
"#;

pub fn activate_command(args: &[String]) -> Fallible<()> {
    let mut opts = getopts::Options::new();
    opts.optflag(
        "n",
        "dry-run",
        "Instead of actually activating the mod, print the actions `modman activate` would take.",
    );

    if args.len() == 1 && args[0] == "help" {
        print_usage(USAGE, &opts);
    }

    let matches = match opts.parse(args) {
        Ok(m) => m,
        Err(f) => {
            eprintln!("{}", f.to_string());
            eprint_usage(USAGE, &opts);
        }
    };

    if matches.free.is_empty() {
        eprint_usage(USAGE, &opts);
    }

    let dry_run = matches.opt_present("n");

    let mut p = load_and_check_profile()?;

    for mod_name in matches.free {
        info!("Activating {}...", mod_name);

        let mod_path = Path::new(&mod_name);

        // First sanity check: we haven't already added this mod.
        if p.mods.contains_key(mod_path) {
            bail!("{} has already been activated!", mod_name);
        }

        apply_mod(mod_path, &mut p, dry_run)?;
    }

    if dry_run {
        print_profile(&p)?;
    }

    Ok(())
}

/// Given a mod's path and a profile, apply a given mod.
/// If dry_run is set, no writes are made.
fn apply_mod(mod_path: &Path, p: &mut Profile, dry_run: bool) -> Fallible<()> {
    let mut m = open_mod(mod_path)?;

    let mod_file_paths = m.paths()?;

    // Look at all the paths we currently have,
    // and make sure the new file doesn't contain any of them.
    check_for_profile_conflicts(mod_path, &mod_file_paths, &p)?;

    // We want to install mod files in a way that minimizes the risk of
    // losing data if this program is interrupted or crashes.
    // So:
    // 1. For each file we want to add or overwrite,
    //    first make a jouranl entry.
    //    (This wouldn't be necessary if we were only overwriting files,
    //    but without a journal, there's no way to know what files we've
    //    added to the game directory if this gets interrupted.)
    // 2. Then, back it up to a temporary file
    //    (and sync it, for what that's worth).
    // 3. Once it's completed, move this temporary file to its actual path
    //    in the backup directory. Since moves are as close as we can get
    //    to atomic ops on the filesystem, this should guarantee that
    //    the backup directory only contains _complete_ copies of files
    //    we've replaced.
    // 4. Then, overwrite the original location with our mod file.
    // 5. Once we've done so for all files, we'll write out the updated profile.
    //
    // If any of this is interrupted, the profile won't mention the mod
    // we were activating or its files, but any overwritten files will have
    // their backups.
    // We should then be able to restore those later.

    let mut journal = create_journal(dry_run)?;

    // We'll add this to the profile once we've applied all files.
    let mut manifest = ModManifest {
        version: m.version().clone(),
        files: BTreeMap::new(),
    };

    for mod_file_path in mod_file_paths {
        let original_hash: Option<FileHash> =
            try_hash_and_backup(&mod_file_path, &p, &mut *journal, dry_run)?;

        // Open and hash the mod file.
        // If this isn't a dry run, overwrite the game file.
        let full_mod_path: String = mod_path
            .join(mod_file_path.as_path())
            .to_string_lossy()
            .into_owned();
        let mut mod_file_reader = BufReader::new(m.read_file(&mod_file_path)?);
        let mod_hash = if dry_run {
            // We don't need to write the mod file anywhere, so just hash it.
            hash_contents(&mut mod_file_reader)
        } else {
            let game_file_path = mod_path_to_game_path(&mod_file_path, &p.root_directory);

            debug!(
                "Installing {} to {}",
                full_mod_path,
                game_file_path.to_string_lossy()
            );

            // Create any needed directory structure.
            let game_file_dir = game_file_path.parent().unwrap();
            create_dir_all(&game_file_dir).map_err(|e| {
                e.context(format!(
                    "Couldn't create directory {}",
                    game_file_dir.to_string_lossy()
                ))
            })?;

            let mut game_file = File::create(&game_file_path).map_err(|e| {
                e.context(format!(
                    "Couldn't overwrite {}",
                    game_file_path.to_string_lossy()
                ))
            })?;

            hash_and_write(&mut mod_file_reader, &mut game_file)
        }?;

        trace!("Mod file {} hashed to\n{:x}", full_mod_path, mod_hash.bytes);

        let meta = ModFileMetadata {
            mod_hash,
            original_hash,
        };

        manifest.files.insert(mod_file_path, meta);
    }

    // Update our profile with a manifest of the mod we just applied.
    p.mods.insert(mod_path.to_owned(), manifest);

    // If it's not a dry run, overwrite the profile file
    // after each mod we apply.
    if !dry_run {
        update_profile_file(&p)?;
        // With that successfully done, we can axe the journal.
        delete_journal(journal)?;
    }

    Ok(())
}

/// Checks the given profile for file paths from a mod we wish to apply,
/// and returns an error if it already contains them.
fn check_for_profile_conflicts(
    mod_path: &Path,
    mod_file_paths: &[PathBuf],
    p: &Profile,
) -> Fallible<()> {
    for mod_file_path in mod_file_paths {
        for (active_mod_name, active_mod) in &p.mods {
            if active_mod.files.contains_key(&*mod_file_path) {
                bail!(
                    "{} from {} would overwrite the same file from {}",
                    mod_file_path.to_string_lossy(),
                    mod_path.to_string_lossy(),
                    active_mod_name.to_string_lossy()
                );
            }
        }
    }
    Ok(())
}

/// Given a mod file's path, back up the game file if one exists.
/// Returns the hash of the game file, or None if no file existed at that path.
/// If dry_run is set, just hash and don't actually backup.
fn try_hash_and_backup(
    mod_file_path: &Path,
    p: &Profile,
    journal: &mut dyn Journal,
    dry_run: bool,
) -> Fallible<Option<FileHash>> {
    let game_file_path = mod_path_to_game_path(mod_file_path, &p.root_directory);

    // Try to open a file in the game directory at mod_file_path,
    // to see if it exists.
    match File::open(&game_file_path) {
        Err(open_err) => {
            // If there's no file there, great. Less work for us.
            if open_err.kind() == std::io::ErrorKind::NotFound {
                debug!(
                    "{} doesn't exist, no need for backup.",
                    game_file_path.to_string_lossy()
                );
                journal.add_file(mod_file_path)?;
                Ok(None)
            }
            // If open() gave a different error, cough that up.
            else {
                Err(Error::from(open_err.context(format!(
                    "Couldn't open {}",
                    game_file_path.to_string_lossy()
                ))))
            }
        }
        Ok(game_file) => {
            journal.replace_file(mod_file_path)?;
            let mut br = BufReader::new(game_file);

            let hash = if !dry_run {
                hash_and_backup(
                    mod_file_path,
                    &game_file_path,
                    &mut br,
                    BackupBehavior::FirstBackup,
                )
            } else {
                hash_contents(&mut br)
            }?;
            trace!(
                "Game file {} hashed to\n{:x}",
                game_file_path.to_string_lossy(),
                hash.bytes
            );
            Ok(Some(hash))
        }
    }
}
