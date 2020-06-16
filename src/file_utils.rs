//! File and directory manipulation utilities

use std::fs;
use std::io::{self, prelude::*};
use std::path::*;

use anyhow::*;
use log::*;
use sha2::*;

use crate::profile::*;

pub fn hash_file(path: &Path) -> Result<FileHash> {
    trace!("Hashing {}", path.display());
    let mut f =
        fs::File::open(&path).with_context(|| format!("Couldn't open {}", path.display()))?;
    hash_contents(&mut f)
}

struct HashingReader<R> {
    inner: R,
    hasher: Sha224,
}

impl<R: Read> HashingReader<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            hasher: Sha224::new(),
        }
    }

    fn result(self) -> FileHash {
        FileHash::new(self.hasher.result())
    }
}

impl<R: Read> Read for HashingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read_result = self.inner.read(buf);
        if let Ok(count) = read_result {
            self.hasher.input(&buf[..count]);
        }
        read_result
    }
}

/// Hash data from the given buffered reader.
/// Mostly used for dry runs where we want to compute hashes but skip backups.
/// (See hash_and_backup() for the real deal.)
pub fn hash_contents<R: Read>(reader: &mut R) -> Result<FileHash> {
    hash_and_write(reader, &mut io::sink())
}

pub fn hash_and_write<R: Read, W: Write>(from: &mut R, to: &mut W) -> Result<FileHash> {
    let mut hasher = HashingReader::new(from);
    io::copy(&mut hasher, to)?;
    Ok(hasher.result())
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

pub fn remove_empty_parents(mut p: &Path, up_to: &Path) -> Result<()> {
    while let Some(parent) = p.parent() {
        // Avoid removing the root directory entirely on a clean sweep.
        if *parent == *up_to {
            return Ok(());
        }
        let removal = fs::remove_dir(&parent);
        if let Err(e) = removal {
            return match e.kind() {
                // If we're doing removes in parallel, there's a chance
                // another thread got it already
                io::ErrorKind::NotFound => Ok(()),
                // If the directory isn't empty...
                io::ErrorKind::Other => {
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
                io::ErrorKind::PermissionDenied => {
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
