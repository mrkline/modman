use std::fs::*;
use std::io::BufReader;

use failure::*;
use log::*;

use crate::file_utils::*;
use crate::profile::*;
use crate::usage::*;

static USAGE: &str = r#"Usage: modman check [options]

Check for possible problems with installed mods and backed up files.
"#;

pub fn check_command(args: &[String]) -> Fallible<()> {
    let opts = getopts::Options::new();

    if args.len() == 1 && args[0] == "help" {
        print_usage(USAGE, &opts);
    }

    // TODO: Allow user to specify a subset of things to check?
    let _matches = match opts.parse(args) {
        Ok(m) => m,
        Err(f) => {
            eprintln!("{}", f.to_string());
            eprint_usage(USAGE, &opts);
        }
    };

    let p = load_and_check_profile()?;

    let mut ok = true;

    info!("Checking if `modman activate` was interrupted...");
    ok &= check_for_journal();

    info!("Verifying backup files...");
    ok &= verify_backups(&p)?;

    info!("Verifying installed mod files...");
    ok &= verify_installed_mod_files(&p)?;

    if ok {
        Ok(())
    } else {
        Err(format_err!("Checks failed!"))
    }
}

fn check_for_journal() -> bool {
    if crate::journal::get_journal_path().exists() {
        warn!(
            "A journal file was found in the backup directory.\n\
            This usually happens when `modman activate` is interrupted \
            before it can update the profile file.\n\
            Run `modman repair` to restore files to the game directory \
            and run `modman activate` again."
        );
        false
    } else {
        true
    }
}

/// Verifies integrity of backup files,
/// and returns false if any fail their check.
fn verify_backups(p: &Profile) -> Fallible<bool> {
    let mut ret = true;

    for manifest in p.mods.values() {
        for (mod_path, metadata) in &manifest.files {
            // If there was no backup, there's nothing to check.
            if metadata.original_hash.is_none() {
                continue;
            }
            let original_hash = &metadata.original_hash.unwrap();

            let backup_path = mod_path_to_backup_path(&**mod_path);
            let f = File::open(&backup_path).map_err(|e| {
                e.context(format!("Couldn't open {}", backup_path.to_string_lossy()))
            })?;
            trace!("Hashing {}", backup_path.to_string_lossy());
            let backup_hash = hash_contents(&mut BufReader::new(f))?;
            if backup_hash != *original_hash {
                debug!(
                    "{} hashed to\n{:x},\nexpected {:x}",
                    backup_path.to_string_lossy(),
                    backup_hash.bytes,
                    original_hash.bytes
                );
                warn!(
                    "The backup of {} has changed!\n\
                     Please repair your game files, then run `modman update` \
                     to make new backups.",
                    mod_path.to_string_lossy()
                );
                ret = false;
            }
        }
    }

    Ok(ret)
}

/// Verifies integrity of installed mod files,
/// and returns false if any fail their check.
fn verify_installed_mod_files(p: &Profile) -> Fallible<bool> {
    let mut ret = true;

    for manifest in p.mods.values() {
        for (mod_path, metadata) in &manifest.files {
            let game_path = mod_path_to_game_path(&**mod_path, p);
            let f = File::open(&game_path)
                .map_err(|e| e.context(format!("Couldn't open {}", game_path.to_string_lossy())))?;
            trace!("Hashing {}", game_path.to_string_lossy());
            let game_hash = hash_contents(&mut BufReader::new(f))?;
            if game_hash != metadata.mod_hash {
                debug!(
                    "{} hashed to\n{:x},\nexpected {:x}",
                    game_path.to_string_lossy(),
                    game_hash.bytes,
                    metadata.mod_hash.bytes
                );
                warn!(
                    "{} has changed!\n\
                     If the game has been updated, run `modman update` \
                     to update backups and reinstall needed files.",
                    game_path.to_string_lossy()
                );
                ret = false;
            }
        }
    }

    Ok(ret)
}
