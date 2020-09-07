use std::fs::File;
use std::io::Read;
use std::path::*;

use self::piz::FileTree;
use anyhow::*;
use memmap::Mmap;
use owning_ref::OwningHandle;
use piz::read as piz;
use semver::Version;

use crate::modification::Mod;

type ZipArchiveHandle = OwningHandle<Box<Mmap>, Box<piz::ZipArchive<'static>>>;
type FileTreeHandle = OwningHandle<ZipArchiveHandle, Box<piz::DirectoryContents<'static>>>;

pub struct ZipMod {
    tree: FileTreeHandle,

    /// The base mod directory name, which we need to strip off of all paths.
    base_dir: &'static piz::Directory<'static>,

    v: Version,

    r: String,
}

impl ZipMod {
    pub fn new(zip_path: &Path) -> Result<Self> {
        let file = File::open(zip_path)?;
        let mmap = Box::new(unsafe { Mmap::map(&file)? });

        let archive = OwningHandle::try_new(mmap, unsafe {
            |map| piz::ZipArchive::new(map.as_ref().unwrap()).map(Box::new)
        })?;
        let tree = OwningHandle::try_new(archive, unsafe {
            |ar| piz::as_tree(ar.as_ref().unwrap().entries()).map(Box::new)
        })?;

        let mut version_info: Option<Version> = None;

        let mut readme: Option<String> = None;

        let mut base_dir: *const piz::Directory = std::ptr::null();

        for (path, entry) in tree.iter() {
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
                    let z = tree.as_owner();
                    let mut vf = z
                        .read(entry.metadata())
                        .context("Couldn't open VERSION.txt")?;
                    let mut version_string = String::new();
                    vf.read_to_string(&mut version_string)?;
                    version_info = Some(
                        Version::parse(&version_string).context("Couldn't parse version string")?,
                    );
                }
                "README.txt" => {
                    assert!(readme.is_none());
                    let z = tree.as_owner();
                    let mut rf = z
                        .read(entry.metadata())
                        .context("Couldn't open README.txt")?;
                    let mut readme_string = String::new();
                    rf.read_to_string(&mut readme_string)?;
                    readme = Some(readme_string);
                }
                _ => {
                    if let piz::DirectoryEntry::Directory(dir) = entry {
                        if base_dir.is_null() {
                            base_dir = &*dir;
                        } else {
                            bail!(
                                "{} contains more than one base directory.",
                                zip_path.display()
                            );
                        }
                    } else {
                        bail!(
                            "{} contains files root besides README.txt and VERSION.txt.",
                            zip_path.display()
                        );
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
        if base_dir.is_null() {
            bail!("Couldn't find a base directory");
        }

        Ok(Self {
            tree,
            // the pointee doesn't move. It lives in a Box -
            // see [`owning_ref::StableAddress`](http://kimundi.github.io/owning-ref-rs/owning_ref/trait.StableAddress.html)
            // - so moving Self won't invalidate the address.
            // We make the lifetime of this reference `&'static` because there's
            // no lifetime to tag it with, so handing that reference to other
            // code would be quite unsafe... but we have no reason to.
            base_dir: unsafe { &base_dir.as_ref().unwrap() },
            v: version_info.unwrap(),
            r: readme.unwrap(),
        })
    }

    fn zip_archive(&self) -> &piz::ZipArchive {
        self.tree.as_owner()
    }
}

impl Mod for ZipMod {
    fn paths(&self) -> Result<Vec<PathBuf>> {
        Ok(self
            .base_dir
            .children
            .files()
            .map(|d| {
                let whole_path = d.path.as_ref();
                let base_dir_path = self.base_dir.metadata.path.as_ref();
                let sans_base_dir = whole_path.strip_prefix(base_dir_path).unwrap();
                PathBuf::from(sans_base_dir)
            })
            .collect())
    }

    fn read_file<'a>(&'a self, p: &Path) -> Result<Box<dyn Read + Send + 'a>> {
        let metadata = self.base_dir.children.lookup(p)?;
        let reader = self.zip_archive().read(metadata)?;
        Ok(reader)
    }

    fn version(&self) -> &Version {
        &self.v
    }

    fn readme(&self) -> &str {
        &self.r
    }
}
