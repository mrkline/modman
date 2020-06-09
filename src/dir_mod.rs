use std::fs;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::*;

use anyhow::*;
use semver::Version;

use crate::file_utils::collect_file_paths_in_dir;
use crate::modification::Mod;

pub struct DirectoryMod {
    base_dir: PathBuf,
    v: Version,
    r: String,
}

impl DirectoryMod {
    pub fn new(path: &Path) -> Result<Self> {
        let dir_iter = fs::read_dir(path)
            .with_context(|| format!("Could not read directory {}", path.display()))?;

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
                    let mut vf =
                        fs::File::open(entry.path()).context("Couldn't open VERSION.txt")?;
                    let mut version_string = String::new();
                    vf.read_to_string(&mut version_string)?;
                    version_info = Some(
                        Version::parse(&version_string).context("Couldn't parse version string")?,
                    );
                }
                "README.txt" => {
                    assert!(readme.is_none());
                    let mut rf =
                        fs::File::open(entry.path()).context("Couldn't open README.txt")?;
                    let mut readme_string = String::new();
                    rf.read_to_string(&mut readme_string)?;
                    readme = Some(readme_string);
                }
                _ => {
                    if entry.file_type()?.is_dir() && base_dir.is_none() {
                        base_dir = Some(entry.path());
                    } else {
                        bail!("{} contains things besides a README.txt, a VERSION.txt, and one base directory.",
                                           path.display());
                    }
                }
            };
        }

        if version_info.is_none() {
            bail!("Couldn't find VERSION.txt");
        }
        if readme.is_none() {
            bail!("Couldn't find README.txt");
        }
        if base_dir.is_none() {
            bail!("Couldn't find a base directory");
        }

        Ok(DirectoryMod {
            base_dir: base_dir.unwrap(),
            v: version_info.unwrap(),
            r: readme.unwrap(),
        })
    }
}

impl Mod for DirectoryMod {
    fn paths(&self) -> Result<Vec<PathBuf>> {
        collect_file_paths_in_dir(&self.base_dir)
    }

    fn read_file(&self, p: &Path) -> Result<Box<dyn Read>> {
        let whole_path = self.base_dir.join(p);
        let f = fs::File::open(&whole_path)
            .with_context(|| format!("Couldn't open mod file ({})", whole_path.display()))?;
        Ok(Box::new(BufReader::new(f)))
    }

    fn version(&self) -> &Version {
        &self.v
    }

    fn readme(&self) -> &str {
        &self.r
    }
}
