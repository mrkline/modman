use std::fs::*;
use std::io::prelude::*;
use std::path::*;

use failure::*;
use log::*;
use sha2::*;

use crate::profile::*;

pub fn hash_file(path: &Path) -> Fallible<FileHash> {
    trace!("Hashing {}", path.to_string_lossy());
    let f = std::fs::File::open(&path)
        .map_err(|e| e.context(format!("Couldn't open {}", path.to_string_lossy())))?;
    hash_contents(&mut std::io::BufReader::new(f))
}

#[derive(PartialEq, Eq, Debug)]
pub enum BackupBehavior {
    FirstBackup,
    ReplaceExisting
}

/// Given a mod file's path and a reader of the game file it's replacing,
/// backup said game file and return its hash.
pub fn hash_and_backup<R: BufRead>(mod_file_path: &Path, reader: &mut R, behavior: BackupBehavior) -> Fallible<FileHash> {
    // First, copy the file to a temporary location, hashing it as we go.
    let temp_file_path = mod_path_to_temp_path(mod_file_path);
    let temp_hash = hash_and_write_temporary(&temp_file_path, reader)?;

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
    if behavior == BackupBehavior::FirstBackup && backup_path.exists() {
        // TODO: Offer corrective action once `modman rescue`
        // or whatever we want to call it exists.
        return Err(format_err!(
            "{} already exists (was `modman activate` previously interrupted?)",
            backup_path.to_string_lossy()
        ));
    }

    trace!(
        "Renaming {} to {}",
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

    Ok(temp_hash)
}

/// Given a path for a temporary file and a buffered reader of the game file it's replacing,
/// copy the game file to our temp directory,
/// then return its hash
fn hash_and_write_temporary<R: BufRead>(
    temp_file_path: &Path,
    reader: &mut R,
) -> Fallible<FileHash> {
    trace!(
        "Hashing and copying to temp file {}",
        temp_file_path.to_string_lossy()
    );

    // Because it's a temp file, we're fine if this truncates an existing file.
    let mut temp_file = File::create(&temp_file_path).map_err(|e| {
        e.context(format!(
            "Couldn't create {}",
            temp_file_path.to_string_lossy()
        ))
    })?;

    let hash = hash_and_write(reader, &mut temp_file)?;

    // sync() is a dirty lie on modern OSes and drives,
    // but do what we can to make sure the data actually made it to disk.
    temp_file.sync_data().map_err(|e| {
        e.context(format!(
            "Couldn't sync {}",
            temp_file_path.to_string_lossy()
        ))
    })?;

    Ok(hash)
}

/// Hash data from the given buffered reader.
/// Mostly used for dry runs where we want to compute hashes but skip backups.
/// (See hash_and_backup() for the real deal.)
pub fn hash_contents<R: BufRead>(reader: &mut R) -> Fallible<FileHash> {
    let mut hasher = Sha224::new();
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

    Ok(FileHash::new(hasher.result()))
}

pub fn hash_and_write<R: BufRead, W: Write>(from: &mut R, to: &mut W) -> Fallible<FileHash> {
    let mut hasher = Sha224::new();

    loop {
        let slice_length = {
            let slice = from.fill_buf()?;
            if slice.is_empty() {
                break;
            }
            to.write_all(slice)?;
            hasher.input(slice);
            slice.len()
        };
        from.consume(slice_length);
    }

    Ok(FileHash::new(hasher.result()))
}

/// Provides a vector of file paths in base_dir, relative to base_dir.
pub fn collect_file_paths_in_dir(base_dir: &Path) -> Fallible<Vec<PathBuf>> {
    let mut ret = Vec::new();
    dir_walker(base_dir, base_dir, &mut ret)?;
    Ok(ret)
}

fn dir_walker(base_dir: &Path, dir: &Path, file_list: &mut Vec<PathBuf>) -> Fallible<()> {
    let dir_iter = read_dir(dir).map_err(|e| {
        e.context(format!(
            "Could not read directory {}",
            dir.to_string_lossy()
        ))
    })?;
    for entry in dir_iter {
        let entry = entry?;
        let ft = entry.file_type()?;
        if ft.is_dir() {
            dir_walker(base_dir, &entry.path(), file_list)?;
        } else if ft.is_file() {
            let entry_path = entry.path();
            let from_base_dir = entry_path.strip_prefix(base_dir)?;
            file_list.push(from_base_dir.to_owned());
        }
        // We don't expect any symbolic links or other unusual things.
        else {
            return Err(format_err!(
                "{} isn't a file or a directory",
                entry.path().to_string_lossy()
            ));
        }
    }
    Ok(())
}

pub fn remove_empty_parents(mut p: &Path) -> Fallible<()> {
    let backup_path = Path::new(crate::profile::BACKUP_PATH);

    while let Some(parent) = p.parent() {
        // Kludge: Avoid removing BACKUP_PATH entirely on a clean sweep.
        if *parent == *backup_path || read_dir(&parent)?.count() > 0 {
            break;
        }
        debug!("Removing empty directory {}", parent.to_string_lossy());
        remove_dir(&parent).map_err(|e| {
            e.context(format!(
                "Couldn't remove empty directory {}",
                parent.to_string_lossy()
            ))
        })?;
        p = parent;
    }
    Ok(())
}
