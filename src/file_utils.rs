use std::fs;
use std::io::prelude::*;
use std::path::*;

use failure::*;
use log::*;
use sha2::*;

use crate::profile::*;

pub fn hash_file(path: &Path) -> Fallible<FileHash> {
    trace!("Hashing {}", path.display());
    let f = fs::File::open(&path).with_context(|_| format!("Couldn't open {}", path.display()))?;
    hash_contents(&mut std::io::BufReader::new(f))
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
    let dir_iter = fs::read_dir(dir)
        .with_context(|_| format!("Could not read directory {}", dir.display()))?;
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
            bail!("{} isn't a file or a directory", entry.path().display());
        }
    }
    Ok(())
}

pub fn remove_empty_parents(mut p: &Path) -> Fallible<()> {
    let backup_path = Path::new(crate::profile::BACKUP_PATH);

    while let Some(parent) = p.parent() {
        // Kludge: Avoid removing BACKUP_PATH entirely on a clean sweep.
        if *parent == *backup_path || fs::read_dir(&parent)?.count() > 0 {
            break;
        }
        debug!("Removing empty directory {}", parent.display());
        fs::remove_dir(&parent)
            .or_else(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    // If we're doing removes in parallel, there's a chance
                    // another thread got it already
                    // warn!("{} was already removed!", parent.display());
                    Ok(())
                } else {
                    Err(e)
                }
            })
            .with_context(|_| format!("Couldn't remove empty directory {}", parent.display()))?;
        p = parent;
    }
    Ok(())
}
