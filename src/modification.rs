use std::fs::metadata;
use std::path::{Path, PathBuf};

use failure::*;

use crate::zip_mod::*;

pub trait Mod {
    fn paths(&mut self) -> Fallible<Vec<PathBuf>>;
}

pub fn open_mod(p: &Path) -> Fallible<Box<dyn Mod>> {
    // Alright, let's stat the thing:
    let stat = metadata(p).map_err(|e| {
        let ctxt = format!("Couldn't find {}", p.to_string_lossy());
        e.context(ctxt)
    })?;

    if stat.is_file() {
        let z = ZipMod::new(p).map_err(|e| {
            let ctxt = format!("Trouble reading {}", p.to_string_lossy());
            e.context(ctxt)
        })?;
        Ok(Box::new(z))
    } else {
        panic!("Directory mods aren't implemented yet.");
    }
}
