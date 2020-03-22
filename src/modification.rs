use std::fs::metadata;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

use failure::*;
use semver::Version;

use crate::dir_mod::*;

pub trait Mod {
    /// Returns a vector of the mod files' paths, with the base directory
    /// stripped away
    fn paths(&self) -> Fallible<Vec<PathBuf>>;

    fn read_file(&self, p: &Path) -> Fallible<Box<dyn BufRead>>;

    fn version(&self) -> &Version;

    fn readme(&self) -> &str;
}

pub fn open_mod(p: &Path) -> Fallible<Box<dyn Mod + Sync>> {
    // Alright, let's stat the thing:
    let stat = metadata(p).with_context(|_| format!("Couldn't find {}", p.display()))?;

    if stat.is_dir() {
        let d = DirectoryMod::new(p)
            .with_context(|_| format!("Trouble reading mod directory {}", p.display()))?;
        Ok(Box::new(d))
    } else {
        Err(format_err!(
            "Couldn't open mod {}: not a directory.",
            p.display()
        ))
    }
}
