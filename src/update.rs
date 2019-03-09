use std::fs::*;
use std::io::BufReader;
use std::path::Path;

use failure::*;
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

pub fn update_command(args: &[String]) -> Fallible<()> {
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

fn update_installed_mods(p: &mut Profile, dry_run: bool) -> Fallible<()> {
    info!("Checking installed mod files...");

    for (mod_path, manifest) in &mut p.mods {
        // First, open up the mod.
        // (If we can't find it, we can't reinstall the mod files.)
        let mut m = open_mod(mod_path)?;

        let current_version: &Version = m.version();
        let activated_version: &Version = &manifest.version;
        if  *current_version != *activated_version {
            return Err(format_err!("{}'s version ({}) doesn't match what it was when ({}) when it was activated",
                mod_path.to_string_lossy(), current_version, activated_version));

        }

        for (mod_file_path, metadata) in &mut manifest.files {

            let mod_file_path: &Path = &**mod_file_path;
            let game_path = mod_path_to_game_path(mod_file_path, &p.root_directory);
            let game_hash = hash_file(&game_path)?;
            if game_hash != metadata.mod_hash {
                debug!(
                    "{} hashed to\n{:x},\nexpected {:x}",
                    game_path.to_string_lossy(),
                    game_hash.bytes,
                    metadata.mod_hash.bytes
                );
                if dry_run {
                    let hash = hash_file(&game_path)?;
                    trace!("Game file {} hashed to\n{:x}", game_path.to_string_lossy(), hash.bytes);
                    println!("{} was changed and needs its backup updated",
                            mod_file_path.to_string_lossy());
                } else {
                    let game_hash = hash_and_backup_file(mod_file_path, &p.root_directory)?;
                    let mut mod_file_reader = BufReader::new(m.read_file(&mod_file_path)?);
                    let game_file_path = mod_path_to_game_path(mod_file_path, &p.root_directory);
                    let mut game_file = File::create(&game_file_path).map_err(|e| {
                        e.context(format!(
                            "Couldn't overwrite {}",
                            game_file_path.to_string_lossy()
                        ))
                    })?;

                    let mod_hash = hash_and_write(&mut mod_file_reader, &mut game_file)?;

                    let full_mod_path: String = mod_path
                        .join(mod_file_path)
                        .to_string_lossy()
                        .into_owned();
                    trace!("Mod file {} hashed to\n{:x}", full_mod_path, mod_hash.bytes);

                    // TODO Update metadata and write it out
                    //
                    metadata.mod_hash = mod_hash;
                    metadata.original_hash = Some(game_hash);
                }
            }
        }
    }
    update_profile_file(&p)?;

    Ok(())
}

/// Given a mod file's path, back up the game file if one exists.
/// Returns the hash of the game file, or None if no file existed at that path.
/// If dry_run is set, just hash and don't actually backup.
fn hash_and_backup_file(mod_file_path: &Path, root_directory: &Path) -> Fallible<FileHash> {
    let game_file_path = mod_path_to_game_path(mod_file_path, root_directory);

    let game_file = File::open(&game_file_path).map_err(|e| {
        e.context(format!(
            "Couldn't open {}",
            game_file_path.to_string_lossy()
        ))
    })?;

    debug!("Backing up {}", game_file_path.to_string_lossy());
    let hash = hash_and_backup(mod_file_path, &mut BufReader::new(game_file))?;

    trace!(
        "Game file {} hashed to\n{:x}",
        game_file_path.to_string_lossy(),
        hash.bytes
    );
    Ok(hash)
}
