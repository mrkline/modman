use std::fs::*;
use std::io::BufReader;
use std::path::Path;

use failure::*;
use log::*;

use crate::file_utils::*;
use crate::profile::*;
use crate::usage::*;

static USAGE: &str = r#"Usage: modman deactivate [options] <MOD>

Deactivate a mod at the path <MOD>.
"#;

pub fn deactivate_command(args: &[String]) -> Fallible<()> {
    let mut opts = getopts::Options::new();
    opts.optflag(
        "n",
        "dry-run",
        "Instead of actually activating the mod, print the actions `modman activate` would take.",
    );

    if args.len() == 1 && args[0] == "help" {
        print_usage(USAGE, &opts);
    }

    let matches = match opts.parse(args) {
        Ok(m) => m,
        Err(f) => {
            eprintln!("{}", f.to_string());
            eprint_usage(USAGE, &opts);
        }
    };

    if matches.free.is_empty() {
        eprint_usage(USAGE, &opts);
    }

    let dry_run = matches.opt_present("n");

    let mut p = load_and_check_profile()?;

    for mod_name in matches.free {
        info!("Deactivating {}...", mod_name);

        let mod_path = Path::new(&mod_name);
        remove_mod(&mod_path, &mut p, dry_run)?;
    }

    if dry_run {
        print_profile(&p)?;
    }

    Ok(())
}

fn remove_mod(mod_path: &Path, p: &mut Profile, dry_run: bool) -> Fallible<()> {
    // First sanity check: this mod is in the profile
    let removed_mod: ModManifest = p.mods.remove(mod_path).ok_or_else(|| {
        return format_err!("{} hasn't been activated.", mod_path.to_string_lossy());
    })?;

    // Everything after this is filesystem work.
    if dry_run {
        return Ok(());
    }

    // We'll do this in a few steps to minimize the chance that backed-up data
    // is lost:
    // 1. Restore all files from backups.
    // 2. Remove mod files that needed no backup.
    // 3. Remove the backups.
    //
    // Unlike activation, we don't need to keep a journal since we don't
    // do anything destructive until we've restored all backups.
    // (TODO: Is applying mods in one pass worth a journal and rescue command?)
    // If we run into issues, tell the user what we've done so far and bail.

    // We could split files that need backups and ones that don't
    // using Iterator::partition(), but it seems simpler to iterate twice
    // instead of allocating storage for partitioned references.

    // Step 1:
    for (file, meta) in removed_mod
        .files
        .iter()
        .filter(|(_f, m)| m.original_hash.is_some())
    {
        restore_file_from_backup(file, meta, p)?;
        // Wait until step 3 to start removing the backups
        // so that we don't mess with backups until
        // the game directory is as it started.
    }

    // Step 2:
    for (file, _) in removed_mod
        .files
        .iter()
        .filter(|(_f, m)| m.original_hash.is_none())
    {
        let game_path = mod_path_to_game_path(file, p);
        debug!("Removing {}", game_path.to_string_lossy());
        // Keep moving if it's already gone,
        // which gets us to step 3 if a previous run of deactivate
        // was interrupted.
        remove_file(&game_path).or_else(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                warn!("{} was already removed!", game_path.to_string_lossy());
                Ok(())
            } else {
                Err(e.context(format!("Couldn't remove {}", game_path.to_string_lossy())))
            }
        })?;
        remove_empty_parents(&game_path)?;
    }

    // Step 3:
    for (file, _) in removed_mod
        .files
        .iter()
        .filter(|(_f, m)| m.original_hash.is_some())
    {
        let backup_path = mod_path_to_backup_path(file);
        debug!("Removing {}", backup_path.to_string_lossy());
        remove_file(&backup_path)
            .map_err(|e| e.context(format!("Couldn't remove {}", backup_path.to_string_lossy())))?;
        remove_empty_parents(&backup_path)?;
    }

    update_profile_file(&p)?;

    Ok(())
}

fn restore_file_from_backup(
    mod_path: &Path,
    mod_meta: &ModFileMetadata,
    p: &Profile,
) -> Fallible<()> {
    assert!(mod_meta.original_hash.is_some());

    let backup_path = mod_path_to_backup_path(mod_path);
    let game_path = mod_path_to_game_path(mod_path, p);
    debug!(
        "Restoring {} to {}",
        backup_path.to_string_lossy(),
        game_path.to_string_lossy()
    );

    // We could use fs::copy(), but let's sanity check that we're putting back
    // the bits we got in the first place.

    let reader = File::open(&backup_path).map_err(|e| {
        e.context(format!(
            "Couldn't open {} to restore it to {}",
            backup_path.to_string_lossy(),
            game_path.to_string_lossy()
        ))
    })?;
    // Because we're restoring contents, this will truncate an existing file.
    let mut game_file = File::create(&game_path).map_err(|e| {
        e.context(format!(
            "Couldn't open {} to overwrite it",
            game_path.to_string_lossy()
        ))
    })?;

    let hash = hash_and_write(&mut BufReader::new(reader), &mut game_file)?;
    trace!(
        "Backup file {} hashed to\n{:x}",
        backup_path.to_string_lossy(),
        hash.bytes
    );
    if hash != mod_meta.original_hash.unwrap() {
        return Err(format_err!(
            "{}'s contents didn't match the hash stored in the profile file
                           when it was restored to {}",
            backup_path.to_string_lossy(),
            game_path.to_string_lossy()
        ));
    }

    Ok(())
}
