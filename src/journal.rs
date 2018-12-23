use std::fs::*;
use std::io::prelude::*;

use std::path::{Path, PathBuf};

use failure::*;

use crate::profile::*;

static JOURNAL_NAME: &str = "activate.journal";

/// A journal (fake or otherwise, see DryRunJournal)
/// that (as best we can, standard caveats apply)
/// records files we're adding or replacing in the game directory.
/// Removed once we've committed those changes to the profile file.
pub trait Journal {
    fn add_file(&mut self, p: &Path) -> Fallible<()> {
        self.entry("Add", p)
    }

    fn replace_file(&mut self, p: &Path) -> Fallible<()> {
        self.entry("Replace", p)
    }

    /// Adds a line to the journal
    fn entry(&mut self, kind: &str, p: &Path) -> Fallible<()>;
}

pub fn create_journal(dry_run: bool) -> Fallible<Box<dyn Journal>> {
    if dry_run {
        Ok(Box::new(DryRunJournal::new()))
    } else {
        let real_deal = ActivationJournal::new()?;
        Ok(Box::new(real_deal))
    }
}

pub fn get_journal_path() -> PathBuf {
    Path::new(TEMPDIR_PATH).join(JOURNAL_NAME).to_owned()
}

pub fn delete_journal(j: Box<dyn Journal>) -> Fallible<()> {
    drop(j);
    let path = get_journal_path();
    let rm = remove_file(&path);
    match rm {
        Ok(()) => Ok(()),
        Err(rm_err) => {
            // No problem if it didn't exist already.
            if rm_err.kind() == std::io::ErrorKind::NotFound {
                Ok(())
            } else {
                Err(Error::from(
                    rm_err.context("Couldn't delete activation journal"),
                ))
            }
        }
    }
}

/// A fake journal that writes to stderr instead of applying sync'd writes
/// to a file.
struct DryRunJournal {}

impl DryRunJournal {
    fn new() -> Self {
        DryRunJournal {}
    }
}

impl Journal for DryRunJournal {
    fn entry(&mut self, kind: &str, p: &Path) -> Fallible<()> {
        let path_str = p.to_string_lossy();
        eprintln!("{} {}", kind, path_str);
        Ok(())
    }
}

struct ActivationJournal {
    fd: File,
}

impl ActivationJournal {
    fn new() -> Fallible<Self> {
        let fd = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(get_journal_path())
            .map_err(|e| {
                let jp = get_journal_path().to_string_lossy().to_string();
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    format_err!(
                        "An activation journal already exists at {}.\n\
                         If a previous run of `modman activate` was interrupted,\n\
                         run `modman repair`.",
                        jp
                    )
                } else {
                    Error::from(e.context("Couldn't create activation journal"))
                }
            })?;
        Ok(ActivationJournal { fd })
    }
}

impl Journal for ActivationJournal {
    /// Adds a line to the journal
    fn entry(&mut self, kind: &str, p: &Path) -> Fallible<()> {
        // In all other places, we've used to_string_lossy(),
        // since they're just for user-facing messages.
        // Here, demand that paths be UTF-8,
        // because reading this back in becomes a cross-platform nightmare
        // (thanks, Windows "Unicode" strings!) otherwise.
        let path_str = p.to_str().expect(crate::encoding::UTF8_ONLY);
        self.fd
            .write_all(format!("{} {}\n", kind, path_str).as_bytes())
            .map_err(|e| e.context("Couldn't append to activation journal"))?;
        self.fd
            .sync_data()
            .map_err(|e| e.context("Couldn't sync activation journal"))?;
        Ok(())
    }
}
