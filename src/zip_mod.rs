use std::fs::File;
use std::path::*;

use anyhow::*;
use memmap::Mmap;
use semver::Version;
use piz::read::{Directory, DirectoryEntry, FileTree, ZipArchive};

use crate::modification::Mod;

pub struct ZipMod {
    mmap: Mmap,

    z: ZipArchive<'what_do>,

    /*
    tree: FileTree<'a>,

    /// The base mod directory name, which we need to strip off of all paths.
    base_dir: &'a Directory<'a>,

    v: Version,

    r: String,
    */
}

impl ZipMod<'_> {
    pub fn new(zip_path: &Path) -> Result<Self> {
        let file = File::open(zip_path)?;
        // We'll be doing lots of seeking, so let's memory map the file
        // to save on all the read calls we'd do otherwise.
        let mmap = unsafe { Mmap::map(&file)? };
        let z = ZipArchive::new(&mmap)?;
        /*
        let tree = FileTree::new(z.entries())?;

        let mut version_info: Option<Version> = None;

        let mut readme: Option<String> = None;

        let mut base_dir: Option<&Directory> = None;

        for (path, entry) in &tree.root {

            // TODO: Parcel out into functions
            match &*path.to_string_lossy() {
                // Carve out special exception for .git in case people build
                // mods with Git.
                // TODO: Other exceptions?
                ".git" => {
                    continue;
                }
                "VERSION.txt" => {
                    assert!(version_info.is_none());
                    let mut vf = z.read(entry.metadata())
                        .context("Couldn't open VERSION.txt")?;
                    let mut version_string = String::new();
                    vf.read_to_string(&mut version_string)?;
                    version_info = Some(
                        Version::parse(&version_string).context("Couldn't parse version string")?,
                    );
                }
                "README.txt" => {
                    assert!(readme.is_none());
                    let mut rf = z.read(entry.metadata())
                        .context("Couldn't open README.txt")?;
                    let mut readme_string = String::new();
                    rf.read_to_string(&mut readme_string)?;
                    readme = Some(readme_string);
                }
                _ => {
                    if let DirectoryEntry::Directory(dir) = entry {
                        if base_dir.is_none() {
                            base_dir = Some(&dir);
                        } else {
                            bail!("{} contains more than one base directory.", zip_path.display());
                        }
                    } else {
                        bail!("{} contains files root besides README.txt and VERSION.txt.", zip_path.display());
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
        */

        Ok(ZipMod {
            mmap,
            z,
            /*
            tree,
            base_dir: base_dir.unwrap(),
            v: version_info.unwrap(),
            r: readme.unwrap(),
            */
        })
    }
}

/*
impl Mod for ZipMod<'_> {
    fn paths(&self) -> Result<Vec<PathBuf>> {
        todo!();
    }

    fn read_file(&self, p: &Path) -> Result<Box<dyn Read>> {
        todo!();
    }

    fn version(&self) -> &Version {
        &self.v
    }

    fn readme(&self) -> &str {
        &self.r
    }
}
*/
