use std::path::{Path, PathBuf};

use anyhow::*;
use log::*;

use crate::file_utils::*;
use crate::journal::*;
use crate::profile::*;
use rayon::prelude::*;

pub fn run() -> Result<()> {
    let p = load_and_check_profile()?;

    let mut ok = true;

    ok &= check_for_journal();
    ok &= find_unknown_files(&p)?;
    ok &= verify_backups(&p)?;
    ok &= verify_installed_mod_files(&p)?;

    if ok {
        Ok(())
    } else {
        bail!("Checks failed!")
    }
}

fn check_for_journal() -> bool {
    info!("Checking if `modman add` was interrupted...");
    if crate::journal::get_journal_path().exists() {
        warn!(
            "A journal file was found in the backup directory.\n\
             This usually happens when `modman add` is interrupted \
             before it can update the profile file.\n\
             Run `modman repair` to restore files to the game directory \
             and run `modman add` again."
        );
        false
    } else {
        true
    }
}

/// Returns the mod_file_paths that aren't mentioned in the profile
/// or the journal.
fn collect_unknown_files(
    mod_file_paths: Vec<PathBuf>,
    p: &Profile,
    jm: &JournalMap,
) -> Vec<PathBuf> {
    mod_file_paths
        .into_iter()
        // We want things that aren't mentioned in the journal
        // Or in any of the mod manifests
        .filter(|path| {
            !jm.contains_key(path)
                && !p
                    .mods
                    .values()
                    .any(|manifest| manifest.files.contains_key(path))
        })
        .collect()
}

/// Checks for unknown files, and returns false if any are found.
fn find_unknown_files(p: &Profile) -> Result<bool> {
    info!("Checking for unknown files...");
    let backed_up_files = collect_file_paths_in_dir(Path::new(BACKUP_PATH))?;

    let mut ret = true;

    // Build a list of files that aren't recorded in the profile
    // or journal.
    let journal_files = read_journal()?;

    let unknown_files = collect_unknown_files(backed_up_files, &p, &journal_files);
    if !unknown_files.is_empty() {
        let mut warning = "The following files were found in the backup directory \
                           but aren't known by modman:"
            .to_owned();
        for file in &unknown_files {
            warning += &format!("\n\t{}", file.display());
        }
        warn!("{}", warning);
        ret = false;
    }

    Ok(ret)
}

/// Verifies integrity of backup files,
/// and returns false if any fail their check.
fn verify_backups(p: &Profile) -> Result<bool> {
    info!("Verifying backup files...");
    let mut backups_ok = true;

    for manifest in p.mods.values() {
        backups_ok &= manifest
            .files
            .par_iter()
            .map(|(mod_path, metadata)| {
                let mod_path: &Path = &**mod_path;

                // If there was no backup, there's nothing to check.
                if metadata.original_hash.is_none() {
                    return Ok(true);
                }
                let original_hash = metadata.original_hash.as_ref().unwrap();

                let backup_path = mod_path_to_backup_path(mod_path);
                let backup_hash = hash_file(&backup_path)?;
                if backup_hash != *original_hash {
                    debug!(
                        "{} hashed to\n{:x},\nexpected {:x}",
                        backup_path.display(),
                        backup_hash.bytes,
                        original_hash.bytes
                    );
                    warn!(
                        "The backup of {} has changed!\n\
                     Please repair your game files, then run `modman update` \
                     to make new backups.",
                        mod_path.display()
                    );
                    Ok(false)
                } else {
                    info!("\t{} is unchanged", mod_path.display());
                    Ok(true)
                }
            })
            .reduce(
                || -> Result<bool> { Ok(true) },
                |left, right| Ok(left? && right?),
            )?;
    }

    Ok(backups_ok)
}

/// Verifies integrity of installed mod files,
/// and returns false if any fail their check.
fn verify_installed_mod_files(p: &Profile) -> Result<bool> {
    info!("Verifying installed mod files...");
    let mut installed_files_ok = true;

    for manifest in p.mods.values() {
        installed_files_ok &= manifest
            .files
            .par_iter()
            .map(|(mod_path, metadata)| {
                let game_path = mod_path_to_game_path(&**mod_path, &p.root_directory);
                let game_hash = hash_file(&game_path)?;
                if game_hash != metadata.mod_hash {
                    debug!(
                        "{} hashed to\n{:x},\nexpected {:x}",
                        game_path.display(),
                        game_hash.bytes,
                        metadata.mod_hash.bytes
                    );
                    warn!(
                        "{} has changed!\n\
                     If the game has been updated, run `modman update` \
                     to update backups and reinstall needed files.",
                        game_path.display()
                    );
                    Ok(false)
                } else {
                    info!("\t{} is unchanged", mod_path.display());
                    Ok(true)
                }
            })
            .reduce(
                || -> Result<bool> { Ok(true) },
                |left, right| Ok(left? && right?),
            )?;
    }

    Ok(installed_files_ok)
}
