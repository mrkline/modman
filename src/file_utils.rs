use std::fs;
use std::io::prelude::*;
use std::path::*;

use anyhow::*;
use log::*;
use sha2::*;

use crate::profile::*;

pub fn hash_file(path: &Path) -> Result<FileHash> {
    trace!("Hashing {}", path.display());
    let f = fs::File::open(&path).with_context(|| format!("Couldn't open {}", path.display()))?;
    hash_contents(&mut std::io::BufReader::new(f))
}

/// Hash data from the given buffered reader.
/// Mostly used for dry runs where we want to compute hashes but skip backups.
/// (See hash_and_backup() for the real deal.)
pub fn hash_contents<R: BufRead>(reader: &mut R) -> Result<FileHash> {
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

pub fn hash_and_write<R: BufRead, W: Write>(from: &mut R, to: &mut W) -> Result<FileHash> {
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
pub fn collect_file_paths_in_dir(base_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut ret = Vec::new();
    dir_walker(base_dir, base_dir, &mut ret)?;
    Ok(ret)
}

fn dir_walker(base_dir: &Path, dir: &Path, file_list: &mut Vec<PathBuf>) -> Result<()> {
    let dir_iter =
        fs::read_dir(dir).with_context(|| format!("Couldn't read directory {}", dir.display()))?;
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

pub fn remove_empty_parents(mut p: &Path) -> Result<()> {
    let backup_path = Path::new(crate::profile::BACKUP_PATH);

    while let Some(parent) = p.parent() {
        // Kludge: Avoid removing BACKUP_PATH entirely on a clean sweep.
        if *parent == *backup_path {
            return Ok(());
        }
        let removal = fs::remove_dir(&parent);
        if let Err(e) = removal {
            return match e.kind() {
                // If we're doing removes in parallel, there's a chance
                // another thread got it already
                std::io::ErrorKind::NotFound => Ok(()),
                // If the directory isn't empty...
                std::io::ErrorKind::Other => {
                    let raw_error = e.raw_os_error().expect("No errno");
                    // POSIX can return ENOTEMPTY (39).
                    // Windows seems to return ERROR_DIR_NOT_EMPTY (145)
                    if (cfg!(unix) && raw_error == 39) || (cfg!(windows) && raw_error == 145) {
                        Ok(())
                    } else {
                        Err(Error::from(e))
                    }
                }
                // Windows seems to return access denied (error 5)
                // sometimes as well. Maybe there's an I/O lock while
                // another thread is trying to remove it?
                std::io::ErrorKind::PermissionDenied => {
                    if cfg!(windows) {
                        Ok(())
                    } else {
                        Err(Error::from(e))
                    }
                }
                _ => Err(Error::from(e)),
            }
            .context(format!(
                "Couldn't remove empty directory {}",
                parent.display()
            ));
        } else {
            debug!("Removed empty directory {}", parent.display());
            p = parent;
        }
    }
    unreachable!("remove_empty_parents() got to a filesystem root");
}
