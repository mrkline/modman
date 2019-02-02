use std::fs::*;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::*;

use failure::*;
use semver::Version;

use crate::file_utils::collect_file_paths_in_dir;
use crate::modification::Mod;

pub struct DirectoryMod {
    base_dir: PathBuf,
    v: Version,
    r: String,
}

impl DirectoryMod {
    pub fn new(path: &Path) -> Fallible<Self> {
        let dir_iter = read_dir(path).map_err(|e| {
            e.context(format!(
                "Could not read directory {}",
                path.to_string_lossy()
            ))
        })?;

        let mut version_info: Option<Version> = None;

        let mut readme: Option<String> = None;

        let mut base_dir: Option<PathBuf> = None;

        for entry in dir_iter {
            let entry = entry?;

            let name = entry.file_name();

            // TODO: Parcel out into functions
            match &*name.to_string_lossy() {
                // Carve out special exception for .git in case people build
                // mods with Git.
                // TODO: Other exceptions?
                ".git" => {
                    continue;
                }
                "VERSION.txt" => {
                    assert!(version_info.is_none());
                    let mut vf = File::open(entry.path())
                        .map_err(|e| e.context("Couldn't open VERSION.txt"))?;
                    let mut version_string = String::new();
                    vf.read_to_string(&mut version_string)?;
                    version_info = Some(
                        Version::parse(&version_string).context("Couldn't parse version string")?,
                    );
                }
                "README.txt" => {
                    assert!(readme.is_none());
                    let mut rf = File::open(entry.path())
                        .map_err(|e| e.context("Couldn't open README.txt"))?;
                    let mut readme_string = String::new();
                    rf.read_to_string(&mut readme_string)?;
                    readme = Some(readme_string);
                }
                _ => {
                    if entry.file_type()?.is_dir() && base_dir.is_none() {
                        base_dir = Some(entry.path());
                    } else {
                        return Err(format_err!("{} contains things besides a README.txt, a VERSION.txt, and one base directory.",
                                           path.to_string_lossy()));
                    }
                }
            };
        }

        Ok(DirectoryMod {
            base_dir: base_dir.unwrap(),
            v: version_info.unwrap(),
            r: readme.unwrap(),
        })
    }
}

impl Mod for DirectoryMod {
    fn paths(&mut self) -> Fallible<Vec<PathBuf>> {
        collect_file_paths_in_dir(&self.base_dir)
    }

    fn read_file<'a>(&'a mut self, p: &Path) -> Fallible<Box<dyn Read + 'a>> {
        let whole_path = self.base_dir.join(p);
        let f = File::open(&whole_path).map_err(|e| {
            e.context(format!(
                "Couldn't open mod file ({})",
                whole_path.to_string_lossy()
            ))
        })?;
        Ok(Box::new(BufReader::new(f)))
    }

    fn version(&self) -> &Version {
        &self.v
    }

    fn readme(&self) -> &str {
        &self.r
    }
}
