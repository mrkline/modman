use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use zip::read::ZipArchive;

use crate::modification::Mod;

pub struct ZipMod {
    z: ZipArchive<BufReader<File>>,
}

impl ZipMod {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, failure::Error> {
        let f = File::open(path)?;
        let br = BufReader::new(f);
        let z = ZipArchive::new(br)?;

        // TODO: Look for VERSION.txt
        // TODO: Look for README.txt
        // TODO: Pull out a base directory name.
        //       A pass over every other path to validate them seems wasetful,
        //       especially since users will probably call paths() or
        //       files() (TODO) next. Should those iterators have
        //       Item = Result<PathBuf, failure::Error>?

        Ok(Self { z })
    }
}

impl Mod for ZipMod {
    fn paths<'a>(&'a mut self) -> Box<dyn Iterator<Item = PathBuf> + 'a> {
        // Chain some iterators to pull the paths out of the zip file.
        // Take indexes 0 through the length, then map those to
        // ZipArchive::by_index() calls. We won't be out of bounds,
        // so unwrap that (right? or could that fail for I/O reasons?),
        // then pull out the name.
        Box::new((0..self.z.len()).map(move |idx| self.z.by_index(idx).unwrap().sanitized_name()))
    }
}
