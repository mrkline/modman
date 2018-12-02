use std::collections::*;
use std::default::Default;
use std::path::{Path, PathBuf};
use std::rc::*;

use semver::Version;
use serde_derive::{Deserialize, Serialize};
use sha2::{Digest, Sha224};

use crate::version_serde::*;

pub static PROFILE_PATH: &str = "modman.profile";

// Directories for persisting the files that modman is replacing.
pub static STORAGE_PATH: &str = "modman-backup";
pub static BACKUP_README: &str = "modman-backup/README.txt";
pub static TEMPDIR_PATH: &str = "modman-backup/temp";
pub static BACKUP_PATH: &str = "modman-backup/originals";

pub type Sha224Bytes = generic_array::GenericArray<u8, <Sha224 as Digest>::OutputSize>;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FileHash {
    pub bytes: Sha224Bytes,
}

impl FileHash {
    pub fn new(b: Sha224Bytes) -> Self {
        Self { bytes: b }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    pub root_directory: PathBuf,
    pub mods: BTreeMap<PathBuf, ModManifest>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModManifest {
    #[serde(
        serialize_with = "serialize_version",
        deserialize_with = "deserialize_version"
    )]
    pub version: Version,
    // Use an Rc<PathBuf> because we'll do plenty of copying file paths around.
    // Rc<Path> would be even better, but serde doesn't like unsized things.
    pub files: BTreeMap<Rc<PathBuf>, ModFileMetadata>,
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

pub fn sanity_check_profile(profile: &Profile) -> failure::Fallible<()> {
    if !profile.root_directory.exists() {
        return Err(failure::format_err!(
            "The root directory {} doesn't exist!\n\
             Has it moved since you ran `modman init`?",
            profile.root_directory.to_string_lossy()
        ));
    }

    Ok(())
}

/// Given a relative mod file path,
/// return its game file path, i.e., it appended to the profile's root directory.
pub fn mod_path_to_game_path(mod_path: &Path, profile: &Profile) -> PathBuf {
    profile.root_directory.join(mod_path)
}

/// Given a relative mod file path,
/// return its backup path, i.e., it appended to our backup directory.
pub fn mod_path_to_backup_path(mod_path: &Path) -> PathBuf {
    Path::new(BACKUP_PATH).join(mod_path)
}

/// Given a relative mod file path,
/// return its temporary path, i.e.,
/// its file name appended to our temp directory,
/// with a `.part` suffix.
pub fn mod_path_to_temp_path(mod_path: &Path) -> PathBuf {
    // We're unwrapping that path has a final path component (i.e., a file name.)
    // Very strange things are happening if it doesn't...
    let mut temp_filename: std::ffi::OsString = mod_path.file_name().unwrap().to_owned();
    temp_filename.push(".part");
    Path::new(TEMPDIR_PATH).join(temp_filename)
}
