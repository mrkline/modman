use std::fs::*;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::exit;

use failure::*;
use log::*;
use sha2::*;

use crate::modification::*;
use crate::profile::*;

static USAGE: &str = r#"
Usage: modman activate [options] <MOD>

Activate a mod at the path <MOD>.
Mods can be in two formats:
  1. A directory containing a VERSION.txt file, a README.txt file,
     and a single directory, which will be treated as the root of the mod files.
  2. A .zip archive containing the same.
"#;

fn print_usage() -> ! {
    println!("{}", USAGE);
    exit(0);
}

fn eprint_usage() -> ! {
    eprintln!("{}", USAGE);
    exit(2);
}

pub fn activate_command(args: &[String]) -> Fallible<()> {
    if args.len() == 1 && args[0] == "help" {
        print_usage();
    }

    if args.is_empty() {
        eprint_usage();
    }

    info!("Loading profile...");

    let f = File::open(PROFILE_PATH)
        .map_err(|e| e.context(format!("Couldn't open profile file ({})", PROFILE_PATH)))?;

    let p: Profile = serde_json::from_reader(f).context("Couldn't parse profile file")?;

    for mod_name in args {
        info!("Activating {}...", mod_name);

        let mod_path = Path::new(mod_name);

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

        for mod_file_path in &mod_file_paths {
            // First, make a backup to a temporary file.
            let mut br = BufReader::new(m.read_file(mod_file_path)?);
            let hnt = hash_and_write_temporary(&mut br, &mod_file_path)?;
            hnt.temp_file.sync_data()?;
            drop(hnt.temp_file); // Close the temp file

            let temp_file_path = hnt.temp_file_path;

            // Next, create any needed directory structure.
            let mut mod_file_dir = PathBuf::from(BACKUP_PATH);
            if let Some(parent) = mod_file_path.parent() {
                mod_file_dir.push(parent);
            }
            create_dir_all(&mod_file_dir).map_err(|e| {
                e.context(format!(
                    "Couldn't create directory {}",
                    mod_file_dir.to_string_lossy()
                ))
            })?;

            let backup_path = mod_file_dir.join(mod_file_path.file_name().unwrap());

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

            rename(&temp_file_path, &backup_path).map_err(|e| {
                e.context(format!(
                    "Couldn't rename {} to {}",
                    temp_file_path.to_string_lossy(),
                    backup_path.to_string_lossy()
                ))
            })?;

            // TODO: The real deal. Write the mod file into the game directory.
        }
        // TODO break the above into functions.

        // TODO Add to profile
    }

    Ok(())
}

/// Checks the given profile for file paths from a mod we wish to apply,
/// and returns an error if it already contains them.
fn check_for_profile_conflicts(mod_file_paths: &[PathBuf], p: &Profile) -> Fallible<()> {
    for mod_file_path in mod_file_paths {
        for (active_mod_name, active_mod) in &p.mods {
            if active_mod.files.contains_key(mod_file_path.as_path()) {
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

struct HashedFile {
    pub hash: FileHash,
    pub temp_file: File,
    pub temp_file_path: PathBuf,
}

fn hash_and_write_temporary<R: BufRead, P: AsRef<Path>>(
    mut reader: R,
    path: P,
) -> Fallible<HashedFile> {
    // We're unwrapping that path has a final path component (i.e., a file name.)
    // Very strange things are happening if it doesn't...
    let mut temp_filename: std::ffi::OsString = path.as_ref().file_name().unwrap().to_owned();
    temp_filename.push(".part");

    let temp_file_path = Path::new(TEMPDIR_PATH).join(temp_filename);

    debug!(
        "Copying {} to {}",
        path.as_ref().to_string_lossy(),
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

    let hash = hasher.result();

    trace!("{} hashed to {:x}", temp_file_path.to_string_lossy(), hash);

    Ok(HashedFile {
        hash,
        temp_file,
        temp_file_path,
    })
}
