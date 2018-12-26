use std::io::prelude::*;
use std::path::Path;

use failure::*;
use log::*;
use sha2::*;

use crate::profile::FileHash;

pub fn hash_file(path: &Path) -> Fallible<FileHash> {
    trace!("Hashing {}", path.to_string_lossy());
    let f = std::fs::File::open(&path).map_err(|e| {
        e.context(format!("Couldn't open {}", path.to_string_lossy()))
    })?;
    hash_contents(&mut std::io::BufReader::new(f))
}

/// Hash data from the given buffered reader.
/// Used for dry runs where we want to compute hashes but skip backups.
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
