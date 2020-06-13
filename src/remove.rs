use std::fs;
use std::path::{Path, PathBuf};

use anyhow::*;
use log::*;
use structopt::*;

use crate::file_utils::*;
use crate::profile::*;
use rayon::prelude::*;

/// Uninstalls a mod
///
/// Mod files from <MOD> are removed from the root directory
/// and any files they replaced are restored from backups.
#[derive(Debug, StructOpt)]
#[structopt(verbatim_doc_comment)]
pub struct Args {
    #[structopt(short = "n", long)]
    dry_run: bool,

    #[structopt(name = "MOD", required(true))]
    mod_names: Vec<PathBuf>,
}

pub fn run(args: Args) -> Result<()> {
    let mut p = load_and_check_profile()?;

    for mod_name in args.mod_names {
        info!("Deactivating {}...", mod_name.display());

        let mod_path = Path::new(&mod_name);
        remove_mod(&mod_path, &mut p, args.dry_run)?;
    }

    if args.dry_run {
        print_profile(&p)?;
    }

    Ok(())
}

fn remove_mod(mod_path: &Path, p: &mut Profile, dry_run: bool) -> Result<()> {
    // First sanity check: this mod is in the profile
    let removed_mod: ModManifest = p.mods.remove(mod_path).ok_or_else(|| {
        return format_err!("{} hasn't been activated.", mod_path.display());
    })?;

    // Everything after this is filesystem work.
    if dry_run {
        return Ok(());
    }

    // We'll do this in a few steps to minimize the chance that data
    // is lost:
    // 1. Verify that all the files we installed are unmodified (add flag to skip?)
    // 2. Restore all files from backups.
    // 3. Remove mod files that needed no backup.
    // 4. Remove the mod from the profile.
    // 5. Remove the backups.
    //
    // Unlike activation, we don't need to keep a journal since we don't
    // do anything destructive until we've restored all backups.
    // (TODO: Is applying mods in one pass worth a journal and rescue command?)
    // If we run into issues, tell the user what we've done so far and bail.

    // We could split files that need backups and ones that don't
    // using Iterator::partition(), but it seems simpler to iterate twice
    // instead of allocating storage for partitioned references.
    info!(
        "Checking that all mod files installed by {} are unmodified...",
        mod_path.display()
    );
    let all_intact = removed_mod
        .files
        .par_iter()
        .map(|(file, meta)| {
            let hash_matches =
                meta.mod_hash == hash_file(&mod_path_to_game_path(file, &p.root_directory))?;
            if !hash_matches {
                warn!(
                    "Mod file {} has changed from when it was installed by mod {}",
                    file.display(),
                    mod_path.display()
                );
            }
            Ok(hash_matches)
        })
        .reduce(
            || -> Result<bool> { Ok(true) },
            |left, right| Ok(left? && right?),
        )?;

    if !all_intact {
        bail!("Some installed mod files were changed. Did the game update?");
    }
    info!("All mod files from {} are intact!", mod_path.display());

    // Step 2:
    removed_mod
        .files
        .par_iter()
        .filter(|(_f, m)| m.original_hash.is_some())
        .try_for_each(|(file, meta)| {
            info!("Restoring {}", file.display());
            restore_file_from_backup(file, meta, &p.root_directory)
            // Wait until step 3 to start removing the backups
            // so that we don't mess with backups until
            // the game directory is as it started.
        })?;

    // Step 3:
    removed_mod
        .files
        .par_iter()
        .filter(|(_f, m)| m.original_hash.is_none())
        .try_for_each(|(file, _)| {
            info!("Removing {}", file.display());
            let game_path = mod_path_to_game_path(file, &p.root_directory);
            // Keep moving if it's already gone,
            // which gets us to step 3 if a previous run of deactivate
            // was interrupted.
            fs::remove_file(&game_path)
                .or_else(|e| {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        warn!("{} was already removed!", game_path.display());
                        Ok(())
                    } else {
                        Err(e)
                    }
                })
                .with_context(|| format!("Couldn't remove {}", game_path.display()))?;
            remove_empty_parents(&game_path)
        })?;

    // Step 4:
    update_profile_file(&p)?;

    // Step 5:
    removed_mod
        .files
        .par_iter()
        .filter(|(_f, m)| m.original_hash.is_some())
        .try_for_each(|(file, _)| {
            let backup_path = mod_path_to_backup_path(file);
            debug!("Removing {}", backup_path.display());
            fs::remove_file(&backup_path)
                .with_context(|| format!("Couldn't remove {}", backup_path.display()))?;
            remove_empty_parents(&backup_path)
        })?;

    Ok(())
}

fn restore_file_from_backup(
    mod_path: &Path,
    mod_meta: &ModFileMetadata,
    root_directory: &Path,
) -> Result<()> {
    assert!(mod_meta.original_hash.is_some());

    let backup_path = mod_path_to_backup_path(mod_path);
    let game_path = mod_path_to_game_path(mod_path, root_directory);
    debug!(
        "Restoring {} to {}",
        backup_path.display(),
        game_path.display()
    );

    // We could use fs::copy(), but let's sanity check that we're putting back
    // the bits we got in the first place.

    let mut reader = fs::File::open(&backup_path).with_context(|| {
        format!(
            "Couldn't open {} to restore it to {}",
            backup_path.display(),
            game_path.display()
        )
    })?;
    // Because we're restoring contents, this will truncate an existing file.
    let mut game_file = fs::File::create(&game_path)
        .with_context(|| format!("Couldn't open {} to overwrite it", game_path.display()))?;

    let hash = hash_and_write(&mut reader, &mut game_file)?;
    trace!(
        "Backup file {} hashed to\n{:x}",
        backup_path.display(),
        hash.bytes
    );
    if hash != *mod_meta.original_hash.as_ref().unwrap() {
        warn!(
            "{}'s contents didn't match the hash stored in the profile file
                           when it was restored to {}",
            backup_path.display(),
            game_path.display()
        );
    }

    Ok(())
}
