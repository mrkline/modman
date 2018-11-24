use std::fs::metadata;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

use failure::*;

use crate::zip_mod::*;

pub trait Mod {
    /// Returns a vector of the mod files' paths, with the base directory
    /// stripped away (TODO).
    ///
    /// Originally this was going to return an iterator,
    /// but ownership becomes very tricky when working with ZIP archives,
    /// since the zip crate's ZipArchive needs mutability to seek around
    /// the underlying file.
    fn paths(&mut self) -> Fallible<Vec<PathBuf>>;

    fn read_file<'a>(&'a mut self, p: &Path) -> Fallible<Box<dyn Read + 'a>>;
}

pub fn open_mod(p: &Path) -> Fallible<Box<dyn Mod>> {
    // Alright, let's stat the thing:
    let stat = metadata(p).map_err(|e| {
        e.context(format!("Couldn't find {}", p.to_string_lossy()))
    })?;

    if stat.is_file() {
        let z = ZipMod::new(p).map_err(|e| {
            e.context(format!("Trouble reading {}", p.to_string_lossy()))
        })?;
        Ok(Box::new(z))
    } else {
        panic!("Directory mods aren't implemented yet.");
    }
}
