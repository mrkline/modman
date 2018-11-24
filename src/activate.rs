use std::collections::*;
use std::fs::*;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::rc::*;

use failure::*;
use log::*;
use sha2::*;

use crate::modification::*;
use crate::profile::*;
use crate::usage::*;

static USAGE: &str = r#"
Usage: modman activate [options] <MOD>

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

    info!("Loading profile...");

    let f = File::open(PROFILE_PATH)
        .map_err(|e| e.context(format!("Couldn't open profile file ({})", PROFILE_PATH)))?;

    let mut p: Profile = serde_json::from_reader(f).context("Couldn't parse profile file")?;
    sanity_check_profile(&p)?;

    // Just for dry run reporting at the end
    let mut new_paths = Vec::<Rc<PathBuf>>::new();
    let mut backed_up_paths = Vec::<Rc<PathBuf>>::new();

    for mod_name in matches.free {
        info!("Activating {}...", mod_name);

        let mod_path = Path::new(&mod_name);

        // First sanity check: we haven't already added this mod.
        if p.mods.contains_key(mod_path) {
            return Err(format_err!("{} has already been activated!", mod_name));
        }

        let mut m = open_mod(mod_path)?;

        let mod_file_paths = m.paths()?;

        // Next, look at all the paths we currently have,
        // and make sure the new file doesn't contain any of them.
        check_for_profile_conflicts(&mod_file_paths, &p)?;

        // We want to install mod files in a way that minimizes the risk of
        // losing data if this program is interrupted or crashes,
        // but without writing a journal to some file.
        // So:
        // 1. For each file we to overwrite, first make a backup to a temporary
        //    file (and sync it, for what that's worth).
        // 2. Once it's completed, move this temporary file to its actual path
        //    in the backup directory. Since moves are as close as we can get
        //    to atomic ops on the filesystem, this should guarantee that
        //    the backup directory only contains _complete_ copies of files
        //    we've replaced.
        // 3. Then, overwrite the original location with our mod file.
        // 4. Once we've done so for all files, we'll rewrite the updated profile.
        //
        // If any of this is interrupted, the profile won't mention the mod
        // we were activating or its files, but any overwritten files will have
        // their backups.
        // We should then be able to restore those later.

        let mut manifest = ModManifest {
            version: m.version().clone(),
            files: BTreeMap::new(),
        };

        for mod_file_path in mod_file_paths {
            let mod_file_path = Rc::new(mod_file_path);

            let original_hash: Option<FileHash> =
                try_hash_and_backup(&*mod_file_path, &p, dry_run)?;

            let mod_hash = hash_file(&mut BufReader::new(m.read_file(&*mod_file_path)?))?;

            // TODO: The real deal. Write the mod file into the game directory.

            if dry_run {
                if original_hash.is_some() {
                    backed_up_paths.push(mod_file_path.clone());
                } else {
                    new_paths.push(mod_file_path.clone());
                }
            }

            let meta = ModFileMetadata {
                mod_hash,
                original_hash,
            };

            manifest.files.insert(mod_file_path, meta);
        }

        p.mods.insert(PathBuf::from(mod_name), manifest);

        if dry_run {
            println!("Files to be added:");
            for path in &new_paths {
                println!("\t{}", path.to_string_lossy());
            }

            println!("Files to be replaced:");
            for path in &backed_up_paths {
                println!("\t{}", path.to_string_lossy());
            }

            println!("New profile file:");
            serde_json::ser::to_writer_pretty(std::io::stdout().lock(), &p)
                .context("Couldn't serialize profile to JSON")?;
        }
    }

    Ok(())
}

/// Checks the given profile for file paths from a mod we wish to apply,
/// and returns an error if it already contains them.
fn check_for_profile_conflicts(mod_file_paths: &[PathBuf], p: &Profile) -> Fallible<()> {
    for mod_file_path in mod_file_paths {
        for (active_mod_name, active_mod) in &p.mods {
            if active_mod.files.contains_key(&*mod_file_path) {
                return Err(format_err!(
                    "{} would overwrite a file from {}",
                    mod_file_path.to_string_lossy(),
                    active_mod_name.to_string_lossy()
                ));
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
    dry_run: bool,
) -> Fallible<Option<FileHash>> {
    let game_file_path = mod_path_to_game_path(mod_file_path, &p);

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
            let mut br = BufReader::new(game_file);

            let hash = if !dry_run {
                hash_and_backup(mod_file_path, &mut br)
            } else {
                hash_file(&mut br)
            }?;
            trace!("{} hashed to {:x}", mod_file_path.to_string_lossy(), hash);
            Ok(Some(hash))
        }
    }
}

/// Given a mod file's path and a reader of the game file it's replacing,
/// backup said game file and return its hash.
fn hash_and_backup<R: BufRead>(mod_file_path: &Path, reader: &mut R) -> Fallible<FileHash> {
    // First, copy the file to a temporary location, hashing it as we go.
    let hnt = hash_and_write_temporary(mod_file_path, reader)?;

    let temp_file_path = hnt.temp_file_path;

    // Next, create any needed directory structure.
    let mut backup_file_dir = PathBuf::from(BACKUP_PATH);
    if let Some(parent) = mod_file_path.parent() {
        backup_file_dir.push(parent);
    }
    create_dir_all(&backup_file_dir).map_err(|e| {
        e.context(format!(
            "Couldn't create directory {}",
            backup_file_dir.to_string_lossy()
        ))
    })?;

    let backup_path = backup_file_dir.join(mod_file_path.file_name().unwrap());
    debug_assert!(backup_path == mod_path_to_backup_path(mod_file_path));

    // Fail if the file already exists.
    // (This is a good sign that a previous run was interrupted
    // and the user should try to restore the backed up files.)
    //
    // stat() then rename() seems like a classic TOCTOU blunder
    // (https://en.wikipedia.org/wiki/Time_of_check_to_time_of_use),
    // but:
    //
    // 1. If someone comes in and replaces the contents of
    //    backup_path between this next line and the rename() call,
    //    it's safe to assume that the data in there is gone anyways.
    //
    // 2. Rust (and even POSIX, for that matter) doesn't provide a
    //    cross-platform approach to fail a rename if the destination
    //    already exists, so we'd have to write OS-specific code for
    //    Linux, Windows, and <other POSIX friends>.
    if backup_path.exists() {
        // TODO: Offer corrective action once `modman rescue`
        // or whatever we want to call it exists.
        return Err(format_err!(
            "{} already exists (was `modman activate` previously interrupted?)",
            backup_path.to_string_lossy()
        ));
    }

    debug!(
        "Moving {} to {}",
        temp_file_path.to_string_lossy(),
        backup_path.to_string_lossy(),
    );

    // Move the backup from the temporary location to its final spot
    // in the backup directory.
    rename(&temp_file_path, &backup_path).map_err(|e| {
        e.context(format!(
            "Couldn't rename {} to {}",
            temp_file_path.to_string_lossy(),
            backup_path.to_string_lossy()
        ))
    })?;

    Ok(hnt.hash)
}

/// Hash data from the given buffered reader.
/// Used for dry runs where we want to compute hashes but skip backups.
/// (See hash_and_backup() for the real deal.)
fn hash_file<R: BufRead>(reader: &mut R) -> Fallible<FileHash> {
    let mut hasher = Sha256::new();
    loop {
        let slice_length = {
            let slice = reader.fill_buf()?;
            if slice.is_empty() {
                break;
            }
            hasher.input(slice);
            slice.len()
        };
        reader.consume(slice_length);
    }

    Ok(hasher.result())
}

struct HashedFile {
    hash: FileHash,
    temp_file_path: PathBuf,
}

/// Given a mod file's path and a buffered reader of the game file it's replacing,
/// copy the game file to our temp directory,
/// then return its hash and the temp path.
fn hash_and_write_temporary<R: BufRead>(
    mod_file_path: &Path,
    reader: &mut R,
) -> Fallible<HashedFile> {
    let temp_file_path = mod_path_to_temp_path(mod_file_path);

    debug!(
        "Copying {} to {}",
        mod_file_path.to_string_lossy(),
        temp_file_path.to_string_lossy()
    );

    // Because it's a temp file, we're fine if this truncates an existing file.
    let mut temp_file = File::create(&temp_file_path).map_err(|e| {
        e.context(format!(
            "Couldn't create {}",
            temp_file_path.to_string_lossy()
        ))
    })?;

    let mut hasher = Sha256::new();

    loop {
        let slice_length = {
            let slice = reader.fill_buf()?;
            if slice.is_empty() {
                break;
            }
            temp_file.write_all(slice)?;
            hasher.input(slice);
            slice.len()
        };
        reader.consume(slice_length);
    }

    // sync() is a dirty lie on modern OSes and drives,
    // but do what we can to make sure the data actually made it to disk.
    temp_file.sync_data()?;

    let hash = hasher.result();

    Ok(HashedFile {
        hash,
        temp_file_path,
    })
}
