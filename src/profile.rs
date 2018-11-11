use std::collections::*;
use std::default::Default;
use std::path::{Path, PathBuf};

use semver::Version;
use serde_derive::{Deserialize, Serialize};

use crate::version_serde::*;

pub static PROFILE_PATH: &str = "modman.profile";

pub fn profile_exists() -> bool {
    Path::new(PROFILE_PATH).exists()
}

pub type FileHash = [u8; 32];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    pub root_directory: PathBuf,
    pub mods: BTreeMap<PathBuf, Mod>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mod {
    #[serde(
        serialize_with = "serialize_version",
        deserialize_with = "deserialize_version"
    )]
    pub version: Version,
    pub files: BTreeMap<PathBuf, ModFileMetadata>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModFileMetadata {
    pub mod_hash: FileHash,
    pub original_hash: Option<FileHash>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Meta {
    // I suck as a developer if it takes over 255 tries to get the correct
    // on-disk format.
    pub version: u8,
}

// Always default to the latest version number
impl Default for Meta {
    fn default() -> Self {
        Meta { version: 1 }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProfileFileData {
    pub profile: Profile,
    pub meta: Meta,
}
