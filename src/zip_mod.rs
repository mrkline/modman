use std::fs::File;
use std::io::prelude::*;
use std::path::*;

use failure::*;
use memmap::Mmap;
use zip::read::{ZipArchive, ZipFile};
use zip::result::ZipResult;

use crate::modification::Mod;

pub struct ZipMod {
    /// The underlying zip archive we'll read files out of.
    z: ZipArchive<std::io::Cursor<Mmap>>,

    /// The base mod directory name, which we need to strip off of all paths.
    base_dir: PathBuf,

    /// Since we need to collect file paths to pull out a base directory,
    /// ZipMod::new() will cache them for the first caller to paths().
    cached_paths: Vec<PathBuf>,
}

impl ZipMod {
    pub fn new<P: AsRef<Path>>(path: P) -> Fallible<Self> {
        let file = File::open(path)?;
        // We'll be doing lots of seeking, so let's memory map the file
        // to save on all the read calls we'd do otherwise.
        let mmap = unsafe { Mmap::map(&file)? };
        let mut z = ZipArchive::new(std::io::Cursor::new(mmap))?;

        // TODO: Parse and examine
        z.by_name("VERSION.txt")
            .context("Couldn't find VERSION.txt")?;

        // TODO: Should we demand that it contain something?
        z.by_name("README.txt")
            .context("Couldn't find README.txt")?;

        let mut cached_paths = collect_file_paths(&mut z)?;

        let base_dir = extract_base_directory(&mut cached_paths)?;

        Ok(Self {
            z,
            base_dir,
            cached_paths,
        })
    }
}

impl Mod for ZipMod {
    fn paths(&mut self) -> Fallible<Vec<PathBuf>> {
        // Grab our cached copy, if we're the first caller and it hasn't been used.
        let mut cached_paths = Vec::new();
        std::mem::swap(&mut self.cached_paths, &mut cached_paths);
        if !cached_paths.is_empty() {
            Ok(cached_paths)
        }
        // Otherwise build it from scratch.
        else {
            let mut paths = collect_file_paths(&mut self.z)?;
            // Since we verified as much on init,
            // let's trust that all paths share the base directory.
            remove_base_directory(&mut paths, &self.base_dir)?;
            Ok(paths)
        }
    }

    fn read_file<'a>(&'a mut self, p: &Path) -> Fallible<Box<dyn Read + 'a>> {
        let r = self.z.by_name(&(self.base_dir.join(p)).to_string_lossy()).map_err(|e| {
            let ctxt = format!("Couldn't extract {}", p.to_string_lossy());
            e.context(ctxt)
        })?;
        Ok(Box::new(r))
    }
}

fn collect_file_paths<R: Read + Seek>(z: &mut ZipArchive<R>) -> Fallible<Vec<PathBuf>> {
    // Chain some iterators to pull the paths out of the zip file.
    (0..z.len())
        .filter_map(|idx| {
            let zip_result = z.by_index(idx);
            filter_map_zip_file(zip_result)
        })
        .filter(|zf| {
            if let Ok(path) = zf {
                return path != Path::new("VERSION.txt") && path != Path::new("README.txt");
            }
            true
        })
        .collect::<ZipResult<Vec<PathBuf>>>()
        .map_err(Error::from)
}

fn extract_base_directory(paths: &mut [PathBuf]) -> Fallible<PathBuf> {
    let mut base_dir = PathBuf::new();
    for path in paths.iter() {
        if let Some(Component::Normal(base)) = path.components().next() {
            if base_dir.as_os_str().is_empty() {
                base_dir = Path::new(base).to_owned();
            } else if base_dir != base {
                return Err(format_err!(
                    "The file {} does not have the same base directory ({}) as other files in the mod",
                    path.to_string_lossy(),
                    base_dir.to_string_lossy()
                ));
            }
        } else {
            return Err(format_err!("An empty path was found."));
        }
    }
    assert!(!base_dir.as_os_str().is_empty());
    remove_base_directory(paths, &base_dir)?;
    Ok(base_dir)
}

fn remove_base_directory<P: AsRef<Path>>(
    paths: &mut [PathBuf],
    base_dir: P,
) -> Result<(), StripPrefixError> {
    for path in paths {
        *path = path.strip_prefix(&base_dir)?.to_owned();
    }
    Ok(())
}

/// Converts a ZipFile to its path, or returns None if it was a directory.
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
