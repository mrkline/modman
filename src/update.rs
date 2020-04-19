use std::fs;
use std::path::{Path, PathBuf};

use anyhow::*;
use log::*;
use semver::Version;

use crate::file_utils::*;
use crate::modification::*;
use crate::profile::*;
use crate::usage::*;

static USAGE: &str = r#"Usage: modman update

Checks if installed mod files have been overwritten by a game update,
and if they have, updates the backups and reinstalls the mod files.
"#;

pub fn update_command(args: &[String]) -> Result<()> {
    let mut opts = getopts::Options::new();
    opts.optflag(
        "n",
        "dry-run",
        "Instead of actually activating the mod, print the actions `modman update` would take.",
    );

    if args.len() == 1 && args[0] == "help" {
        print_usage(USAGE, &opts);
    }

    // TODO: Allow user to specify a subset of things to check?
    let matches = match opts.parse(args) {
        Ok(m) => m,
        Err(f) => {
            eprintln!("{}", f.to_string());
            eprint_usage(USAGE, &opts);
        }
    };

    let dry_run = matches.opt_present("n");

    let mut p = load_and_check_profile()?;
    update_installed_mods(&mut p, dry_run)?;

    Ok(())
}

fn update_installed_mods(p: &mut Profile, dry_run: bool) -> Result<()> {
    info!("Checking installed mod files...");

    let mut updates_made = false;

    for (mod_path, manifest) in &mut p.mods {
        // First, open up the mod.
        // (If we can't find it, we can't reinstall the mod files.)
        let m = open_mod(mod_path)?;

        let current_version: &Version = m.version();
        let activated_version: &Version = &manifest.version;
        if *current_version != *activated_version {
            bail!(
                "{}'s version ({}) doesn't match what it was when ({}) when it was activated",
                mod_path.display(),
                current_version,
                activated_version
            );
        }

        for (mod_file_path, metadata) in &mut manifest.files {
            if let Some(new_metadata) = update_file(
                mod_path,
                mod_file_path,
                metadata,
                &*m,
                &p.root_directory,
                dry_run,
            )? {
                updates_made = true;
                *metadata = new_metadata;
            }
        }
        // Ideally we'd like to write out the profile file here,
        // once after each mod we've visited.
        // However, we'd need to borrow p, which has a mutable borrow on it
        // from this loop. What do?
    }

    if updates_made {
        if !dry_run {
            update_profile_file(&p)?;
        }
    } else {
        info!("Game files haven't changed, no updates needed.");
    }

    Ok(())
}

/// The core of update_installed_mods's loop.
/// Given the path of the mod (for tracing purposes),
/// the path of the file to update, that file's metadata,
/// the mod itself (for reinstalling the mod file),
/// the game's root directory, and a dry run flag,
///
/// 1. See if the game file's been changed by an update.
/// 2. If it has,
///    a) copy it to the backup directory
///    b) replace it with the mod file again.
///    c) Update the metadata
///
/// Returns true if the file changed (and was updated), or false if it was not.
///
/// This function could be broken down even more, but it's hard to do that
/// without passing lots of args everywhere.
/// For anything we do, we want a handful of paths for debug and trace statements.
fn update_file(
    mod_path: &Path,
    mod_file_path: &Path,
    old_metadata: &ModFileMetadata,
    m: &dyn Mod,
    root_directory: &Path,
    dry_run: bool,
) -> Result<Option<ModFileMetadata>> {
    let game_path = mod_path_to_game_path(mod_file_path, root_directory);
    let game_hash = hash_file(&game_path)?;
    if game_hash == old_metadata.mod_hash {
        // Cool, nothing changed
        return Ok(None);
    }

    trace!(
        "{} hashed to\n{:x},\nexpected {:x}",
        game_path.display(),
        game_hash.bytes,
        old_metadata.mod_hash.bytes
    );

    if dry_run {
        println!(
            "{} was changed and needs its backup updated",
            mod_file_path.display()
        );
        return Ok(Some(ModFileMetadata {
            mod_hash: old_metadata.mod_hash.clone(),
            original_hash: Some(game_hash),
        }));
    }

    info!(
        "{} changed. Backing up new version and reinstalling mod file.",
        game_path.display()
    );

    backup_file(&game_path, mod_file_path)?;

    // This is very simimlar to what `modman activate` is doing
    // to initially install mods, but it has a few differences
    // (we don't have to worry about a dry run hashing the mod file again,
    // we don't have to create directories, etc.)
    // But should we factor them into a common function to their traces
    // and behavior in sync anyways?
    let mut mod_file_reader = m.read_file(&mod_file_path)?;
    let mut game_file = fs::File::create(&game_path)
        .with_context(|| format!("Couldn't overwrite {}", game_path.display()))?;

    let mod_hash = hash_and_write(&mut mod_file_reader, &mut game_file)?;

    let full_mod_path = mod_path.join(mod_file_path);
    trace!(
        "Mod file {} hashed to\n{:x}",
        full_mod_path.display(),
        mod_hash.bytes
    );

    let new_metadata = ModFileMetadata {
        mod_hash,
        original_hash: Some(game_hash),
    };

    // TODO Update metadata and write it out
    if old_metadata.mod_hash != new_metadata.mod_hash {
        warn!(
            "The mod file {} doesn't hash to what it did last time it was installed!",
            full_mod_path.display()
        );
    }

    Ok(Some(new_metadata))
}

/// Given a mod path, hash and backup the corresponding game file.
/// Like try_hash_and_backup() from `modman activate`, but doesn't have to deal
/// with the possibility that the game file isn't there.
fn backup_file(game_file_path: &Path, mod_file_path: &Path) -> Result<()> {
    debug!("Backing up {}", game_file_path.display());

    // First, copy the file to a temporary location, hashing it as we go.
    let temp_file_path = mod_path_to_temp_path(mod_file_path);
    trace!(
        "Copying {} to {}",
        game_file_path.display(),
        temp_file_path.display()
    );
    fs::copy(game_file_path, &temp_file_path).with_context(|| {
        format!(
            "Couldn't copy {} to {}",
            game_file_path.display(),
            temp_file_path.display()
        )
    })?;

    // Next, create any needed directory structure.
    let mut backup_file_dir = PathBuf::from(BACKUP_PATH);
    if let Some(parent) = mod_file_path.parent() {
        backup_file_dir.push(parent);
    }
    fs::create_dir_all(&backup_file_dir)
        .with_context(|| format!("Couldn't create directory {}", backup_file_dir.display()))?;

    let backup_path = backup_file_dir.join(mod_file_path.file_name().unwrap());
    debug_assert!(backup_path == mod_path_to_backup_path(mod_file_path));

    trace!(
        "Renaming {} to {}",
        temp_file_path.display(),
        backup_path.display(),
    );

    // Move the backup from the temporary location to its final spot
    // in the backup directory.
    fs::rename(&temp_file_path, &backup_path).with_context(|| {
        format!(
            "Couldn't rename {} to {}",
            temp_file_path.display(),
            backup_path.display()
        )
    })?;
    Ok(())
}
