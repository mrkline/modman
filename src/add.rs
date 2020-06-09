use std::collections::*;
use std::fs;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::{mpsc::channel, Mutex};

use anyhow::*;
use log::*;
use rayon::prelude::*;
use structopt::*;

use crate::file_utils::*;
use crate::journal::*;
use crate::modification::*;
use crate::profile::*;

/// Installs a mod.
///
/// Each <MOD> is assumed to be a ZIP archive or a directory containing
/// a VERSION.txt file, a README.txt file, and a single sub-directory,
/// which will be treated as the root of the mod files.
/// (Any JSGME or OVGME-compatible archive should match this format.)
///
/// This command installs all mod files, and if they conflict with ones
/// in the root directory, backs those up.
#[derive(Debug, StructOpt)]
#[structopt(verbatim_doc_comment)]
pub struct Args {
    #[structopt(short = "n", long)]
    dry_run: bool,

    #[structopt(name = "MOD", required(true))]
    mod_names: Vec<PathBuf>,
}

pub fn run(args: Args) -> Result<()> {
    let mut p = load_and_check_profile()?;

    for mod_name in args.mod_names {
        info!("Activating {}...", mod_name.display());

        let mod_path = Path::new(&mod_name);

        // First sanity check: we haven't already added this mod.
        if p.mods.contains_key(mod_path) {
            bail!("{} has already been added!", mod_name.display());
        }

        apply_mod(mod_path, &mut p, args.dry_run)?;
    }

    if args.dry_run {
        print_profile(&p)?;
    }

    Ok(())
}

/// Given a mod's path and a profile, apply a given mod.
/// If dry_run is set, no writes are made.
fn apply_mod(mod_path: &Path, p: &mut Profile, dry_run: bool) -> Result<()> {
    let m = open_mod(mod_path)?;

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

    // We'll add this to the profile once we've applied all files.
    let mut manifest = ModManifest {
        version: m.version().clone(),
        files: BTreeMap::new(),
    };

    let (tx, rx) = channel();

    let journal_mutex = Mutex::new(create_journal(dry_run)?);
    let journal: &Mutex<_> = &journal_mutex;

    mod_file_paths
        .into_par_iter()
        .try_for_each_with::<_, _, Result<()>>(tx, |tx, mod_file_path| {
            let original_hash: Option<FileHash> =
                try_hash_and_backup(&mod_file_path, &p, journal, dry_run)?;

            if original_hash.is_none() {
                info!("Adding {}", mod_file_path.display());
            } else {
                info!("Replacing {}", mod_file_path.display());
            }

            // Open and hash the mod file.
            // If this isn't a dry run, overwrite the game file.
            let full_mod_path = mod_path.join(mod_file_path.as_path());
            let mut mod_file_reader = m.read_file(&mod_file_path)?;
            let mod_hash = if dry_run {
                // We don't need to write the mod file anywhere, so just hash it.
                hash_contents(&mut mod_file_reader)
            } else {
                let game_file_path = mod_path_to_game_path(&mod_file_path, &p.root_directory);

                debug!(
                    "Installing {} to {}",
                    full_mod_path.display(),
                    game_file_path.display()
                );

                // Create any needed directory structure.
                let game_file_dir = game_file_path.parent().unwrap();
                fs::create_dir_all(&game_file_dir).with_context(|| {
                    format!("Couldn't create directory {}", game_file_dir.display())
                })?;

                let mut game_file = fs::File::create(&game_file_path)
                    .with_context(|| format!("Couldn't overwrite {}", game_file_path.display()))?;

                hash_and_write(&mut mod_file_reader, &mut game_file)
            }?;

            trace!(
                "Mod file {} hashed to\n{:x}",
                full_mod_path.display(),
                mod_hash.bytes
            );

            let meta = ModFileMetadata {
                mod_hash,
                original_hash,
            };

            tx.send((mod_file_path.clone(), meta))
                .expect("Couldn't send");
            Ok(())
        })?;

    for path_and_meta in rx {
        manifest.files.insert(path_and_meta.0, path_and_meta.1);
    }

    // Update our profile with a manifest of the mod we just applied.
    p.mods.insert(mod_path.to_owned(), manifest);

    // If it's not a dry run, overwrite the profile file
    // after each mod we apply.
    if !dry_run {
        update_profile_file(&p)?;
        // With that successfully done, we can axe the journal.
        delete_journal(journal_mutex.into_inner().unwrap())?;
    }

    Ok(())
}

/// Checks the given profile for file paths from a mod we wish to apply,
/// and returns an error if it already contains them.
fn check_for_profile_conflicts(
    mod_path: &Path,
    mod_file_paths: &[PathBuf],
    p: &Profile,
) -> Result<()> {
    for mod_file_path in mod_file_paths {
        for (active_mod_name, active_mod) in &p.mods {
            if active_mod.files.contains_key(&*mod_file_path) {
                bail!(
                    "{} from {} would overwrite the same file from {}",
                    mod_file_path.display(),
                    mod_path.display(),
                    active_mod_name.display()
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
    journal: &Mutex<Box<dyn Journal>>,
    dry_run: bool,
) -> Result<Option<FileHash>> {
    let game_file_path = mod_path_to_game_path(mod_file_path, &p.root_directory);

    // Try to open a file in the game directory at mod_file_path,
    // to see if it exists.
    match fs::File::open(&game_file_path) {
        Err(open_err) => {
            // If there's no file there, great. Less work for us.
            if open_err.kind() == std::io::ErrorKind::NotFound {
                debug!(
                    "{} doesn't exist, no need for backup.",
                    game_file_path.display()
                );
                journal.lock().unwrap().add_file(mod_file_path)?;
                Ok(None)
            }
            // If open() gave a different error, cough that up.
            else {
                Err(Error::from(open_err)
                    .context(format!("Couldn't open {}", game_file_path.display())))
            }
        }
        Ok(game_file) => {
            journal.lock().unwrap().replace_file(mod_file_path)?;
            let mut br = BufReader::new(game_file);

            let hash = if !dry_run {
                hash_and_backup(mod_file_path, &game_file_path, &mut br)
            } else {
                hash_contents(&mut br)
            }?;
            trace!(
                "Game file {} hashed to\n{:x}",
                game_file_path.display(),
                hash.bytes
            );
            Ok(Some(hash))
        }
    }
}

/// Given a mod file's path and a reader of the game file it's replacing,
/// backup said game file and return its hash.
/// The game file path is provided to print a uniform debug message,
/// but we take a reader instead of opening the file in here because
/// `modman activate` and `modman update` need to do different things.
/// (The former makes a journal entry, and skips to the next file if we don't
/// need to backup. The latter expects the file to exist.)
fn hash_and_backup<R: Read>(
    mod_file_path: &Path,
    game_file_path: &Path,
    reader: &mut R,
) -> Result<FileHash> {
    debug!("Backing up {}", game_file_path.display());

    // First, copy the file to a temporary location, hashing it as we go.
    let temp_file_path = mod_path_to_temp_path(mod_file_path);
    let temp_hash = hash_and_write_temporary(&temp_file_path, reader)?;

    // Next, create any needed directory structure.
    let mut backup_file_dir = PathBuf::from(BACKUP_PATH);
    if let Some(parent) = mod_file_path.parent() {
        backup_file_dir.push(parent);
    }
    fs::create_dir_all(&backup_file_dir)
        .with_context(|| format!("Couldn't create directory {}", backup_file_dir.display()))?;

    let backup_path = backup_file_dir.join(mod_file_path.file_name().unwrap());
    debug_assert!(backup_path == mod_path_to_backup_path(mod_file_path));

    // Fail if the file already exists and we don't expect it.
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
        bail!(
            "{} already exists (was `modman activate` previously interrupted?)",
            backup_path.display()
        );
    }

    trace!(
        "Renaming {} to {}",
        temp_file_path.display(),
        backup_path.display(),
    );

    // Move the backup from the temporary location to its final spot
    // in the backup directory.
    fs::rename(&temp_file_path, &backup_path).with_context(|| {
        format!(
            "Couldn't rename {} to {}",
            temp_file_path.display(),
            backup_path.display()
        )
    })?;

    Ok(temp_hash)
}

/// Given a path for a temporary file and a buffered reader of the game file it's replacing,
/// copy the game file to our temp directory,
/// then return its hash
fn hash_and_write_temporary<R: Read>(temp_file_path: &Path, reader: &mut R) -> Result<FileHash> {
    trace!(
        "Hashing and copying to temp file {}",
        temp_file_path.display()
    );

    // Because it's a temp file, we're fine if this truncates an existing file.
    let mut temp_file = fs::File::create(&temp_file_path)
        .with_context(|| format!("Couldn't create {}", temp_file_path.display()))?;

    let hash = hash_and_write(reader, &mut temp_file)?;

    // sync() is a dirty lie on modern OSes and drives,
    // but do what we can to make sure the data actually made it to disk.
    temp_file
        .sync_data()
        .with_context(|| format!("Couldn't sync {}", temp_file_path.display()))?;

    Ok(hash)
}
