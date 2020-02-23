use std::fs::metadata;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

use failure::*;
use semver::Version;

use crate::dir_mod::*;

pub trait Mod {
    /// Returns a vector of the mod files' paths, with the base directory
    /// stripped away (TODO).
    ///
    /// Originally this was going to return an iterator,
    /// but ownership becomes very tricky when working with ZIP archives,
    /// since the zip crate's ZipArchive needs mutability to seek around
    /// the underlying file.
    fn paths(&mut self) -> Fallible<Vec<PathBuf>>;

    fn read_file(&mut self, p: &Path) -> Fallible<Box<dyn BufRead + Send>>;

    fn version(&self) -> &Version;

    fn readme(&self) -> &str;
}

pub fn open_mod(p: &Path) -> Fallible<Box<dyn Mod>> {
    // Alright, let's stat the thing:
    let stat = metadata(p).with_context(|_| format!("Couldn't find {}", p.to_string_lossy()))?;

    if stat.is_dir() {
        let d = DirectoryMod::new(p)
            .with_context(|_| format!("Trouble reading mod directory {}", p.to_string_lossy()))?;
        Ok(Box::new(d))
    } else {
        Err(format_err!(
            "Couldn't open mod {}: not a directory.",
            p.to_string_lossy()
        ))
    }
}
