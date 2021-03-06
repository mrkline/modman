use std::fs;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

use anyhow::*;
use semver::Version;

use crate::dir_mod::*;
use crate::zip_mod::*;

pub trait Mod {
    /// Returns a vector of the mod files' paths, with the base directory
    /// stripped away
    fn paths(&self) -> Result<Vec<PathBuf>>;

    fn read_file<'a>(&'a self, p: &Path) -> Result<Box<dyn Read + Send + 'a>>;

    fn version(&self) -> &Version;

    fn readme(&self) -> &str;
}

pub fn open_mod(p: &Path) -> Result<Box<dyn Mod + Sync>> {
    // Alright, let's stat the thing:
    let stat = fs::metadata(p).with_context(|| format!("Couldn't find {}", p.display()))?;

    if stat.is_file() {
        let z =
            ZipMod::new(p).with_context(|| format!("trouble reading mod file {}", p.display()))?;
        Ok(Box::new(z))
    } else if stat.is_dir() {
        let d = DirectoryMod::new(p)
            .with_context(|| format!("Trouble reading mod directory {}", p.display()))?;
        Ok(Box::new(d))
    } else {
        Err(format_err!(
            "Couldn't open mod {}: not a directory.",
            p.display()
        ))
    }
}
