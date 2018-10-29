use std::collections::*;
use std::default::Default;
use std::path::PathBuf;
use semver::Version;
use serde_derive::{Serialize,Deserialize};

use crate::version_serde::*;

type FileHash = [u8; 32];

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Profile {
    pub root_directory: PathBuf,
    pub mods: BTreeMap<String, Mod>
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Mod {
    #[serde(serialize_with="serialize_version", deserialize_with="deserialize_version")]
    pub version: Version,
    pub files: Vec<ModFile>
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct ModFile {
    pub path: PathBuf,
    pub original_hash: FileHash,
    pub game_hash: FileHash
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct Meta {
    // I suck as a developer if it takes over 255 tries to get the correct
    // on-disk format.
    pub version: u8
}

// Always default to the latest version number
impl Default for Meta {
    fn default() -> Self { Meta { version: 1} }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct ProfileFileData {
    pub profile: Profile,
    pub meta: Meta
}
