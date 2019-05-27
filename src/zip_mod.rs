use std::fs::File;
use std::io::prelude::*;
use std::path::*;

use failure::*;
use memmap::Mmap;
use semver::Version;
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

    v: Version,

    r: String,
}

impl ZipMod {
    pub fn new(path: &Path) -> Fallible<Self> {
        let file = File::open(path)?;
        // We'll be doing lots of seeking, so let's memory map the file
        // to save on all the read calls we'd do otherwise.
        let mmap = unsafe { Mmap::map(&file)? };
        let mut z = ZipArchive::new(std::io::Cursor::new(mmap))?;

        let mut version_string = String::new();
        z.by_name("VERSION.txt")
            .context("Couldn't find VERSION.txt")?
            .read_to_string(&mut version_string)?;
        let v = Version::parse(&version_string).context("Couldn't parse version string")?;

        let mut r = String::new();
        z.by_name("README.txt")
            .context("Couldn't find README.txt")?
            .read_to_string(&mut r)?;

        let mut cached_paths = collect_file_paths(&mut z)?;

        let base_dir = extract_base_directory(&mut cached_paths)?;

        Ok(Self {
            z,
            base_dir,
            cached_paths,
            v,
            r,
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
        let r = self
            .z
            .by_name(&(self.base_dir.join(p)).to_string_lossy())
            .map_err(|e| e.context(format!("Couldn't extract {}", p.to_string_lossy())))?;
        Ok(Box::new(r))
    }

    fn version(&self) -> &Version {
        &self.v
    }

    fn readme(&self) -> &str {
        &self.r
    }
}

/// Returns the paths of all files in the zip archive,
/// except for mod metadata (version and readme)
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
        .collect::<Fallible<Vec<PathBuf>>>()
        .map_err(Error::from)
}

/// Finds the common base directory of all given paths,
/// then removes it from each of them.
/// Returns the base directory.
fn extract_base_directory(paths: &mut [PathBuf]) -> Fallible<PathBuf> {
    let mut base_dir = PathBuf::new();
    for path in paths.iter() {
        if let Some(Component::Normal(base)) = path.components().next() {
            if base_dir.as_os_str().is_empty() {
                base_dir = Path::new(base).to_owned();
            } else if base_dir != base {
                bail!(
                    "The file {} does not have the same base directory ({}) as other files in the mod",
                    path.to_string_lossy(),
                    base_dir.to_string_lossy()
                );
            }
        } else {
            bail!("An empty path was found.");
        }
    }
    assert!(!base_dir.as_os_str().is_empty());
    remove_base_directory(paths, &base_dir)?;
    Ok(base_dir)
}

/// Removes base_dir from the front of each path in paths
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
fn filter_map_zip_file(r: ZipResult<ZipFile>) -> Option<Fallible<PathBuf>> {
    // If the ZipResult was an error, return that.
    // We user .err().unwrap() instead of unwrap_err() because apparently
    // ZipFile doesn't implement Debug, which unwrap_err() wants when it panics.
    if r.is_err() {
        return Some(Err(Error::from(r.err().unwrap())));
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

    let name = zip_file.sanitized_name();
    // The file name had better not be empty.
    // Checking this now will save us lots of trouble trying to process it later.
    if name.file_name().is_none() {
        return Some(Err(format_err!("File with no name found in ZIP archive")));
    }

    Some(Ok(zip_file.sanitized_name()))
}
