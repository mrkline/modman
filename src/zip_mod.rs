use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use failure::*;
use zip::read::{ZipArchive, ZipFile};
use zip::result::ZipResult;

use crate::modification::Mod;

pub struct ZipMod {
    z: ZipArchive<BufReader<File>>,
}

impl ZipMod {
    pub fn new<P: AsRef<Path>>(path: P) -> Fallible<Self> {
        let f = File::open(path)?;
        let br = BufReader::new(f);
        let mut z = ZipArchive::new(br)?;

        // TODO: Parse and examine
        z.by_name("VERSION.txt")
            .context("Couldn't find VERSION.txt")?;

        // TODO: Should we demand that it contain something?
        z.by_name("README.txt")
            .context("Couldn't find README.txt")?;

        // TODO: Pull out a base directory name.
        //       A pass over every other path to validate them seems wasetful,
        //       especially since users will probably call paths() or
        //       files() (TODO) next. Should those iterators have
        //       Item = Result<PathBuf, failure::Error>?

        Ok(Self { z })
    }
}

impl Mod for ZipMod {
    fn paths(&mut self) -> Fallible<Vec<PathBuf>> {
        // Chain some iterators to pull the paths out of the zip file.
        (0..self.z.len())
            .filter_map(|idx| {
                let zip_result = self.z.by_index(idx);
                filter_map_zip_file(zip_result)
            })
            .collect::<ZipResult<Vec<PathBuf>>>()
            .map_err(Error::from)
    }
}

// Converts ZipFile to its path, filtering out directories.
fn filter_map_zip_file(r: ZipResult<ZipFile>) -> Option<ZipResult<PathBuf>> {
    // If the ZipResult was an error, return that.
    // We user .err().unwrap() instead of unwrap_err() because apparently
    // ZipFile doesn't implement Debug, which unwrap_err() wants when it panics.
    if r.is_err() {
        return Some(Err(r.err().unwrap()));
    }

    let zip_file = r.unwrap();
    // The zip crate seems to give us no way to differentiate between
    // a directory and a file in the ZIP archive except by looking at mode bits.
    // For directories, it always seems to set S_IFDIR, i.e., o40000.
    if let Some(mode_bits) = zip_file.unix_mode() {
        if (mode_bits & 0o40000) != 0 {
            return None;
        }
    }
    Some(Ok(zip_file.sanitized_name()))
}
