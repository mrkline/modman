use std::fs::*;
use std::io::BufReader;
use std::path::*;

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

    let matches = match opts.parse(args) {
        Ok(m) => m,
        Err(f) => {
            eprintln!("{}", f.to_string());
            eprint_usage(USAGE, &opts);
        }
    };

    let p = load_and_check_profile()?;

    let mut ok = true;

    info!("Searching backup directory for unknown files...");
    ok &= find_unknown_files(&p)?;

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

/// Checks for unknown files, and returns false if any are found.
fn find_unknown_files(p: &Profile) -> Fallible<bool> {
    let backed_up_files = collect_file_paths_in_backup()?;

    let mut ret = true;

    // Build a list of files that aren't recorded in the profile.
    let unknown_files = collect_unknown_files(backed_up_files, &p);
    if !unknown_files.is_empty() {
        let mut warning = format!(
        "The following files were found in the backup directory, \
            but aren't recorded in the profile file:\n");
        for file in &unknown_files {
            warning += &format!("\t{}\n", file.to_string_lossy());
        }
        warning += "This usually happens when `modman actviate` is interrupted \
            before it can update the profile file.\n\
            Run `modman repair` to restore these files to the game directory \
            and run `modman activate` again.";
        warn!("{}", warning);
        ret = false;
    }

    Ok(ret)
}

fn collect_file_paths_in_backup() -> Fallible<Vec<PathBuf>> {
    let mut ret = Vec::new();
    backup_dir_walker(Path::new(BACKUP_PATH), &mut ret)?;
    Ok(ret)
}

fn backup_dir_walker(dir: &Path, file_list: &mut Vec<PathBuf>) -> Fallible<()> {
    let dir_iter = read_dir(dir).map_err(|e|
        e.context(format!("Could not read directory {}", dir.to_string_lossy())))?;
    for entry in dir_iter {
        let entry = entry?;
        let ft = entry.file_type()?;
        if ft.is_dir() {
            backup_dir_walker(&entry.path(), file_list)?;
        }
        else if ft.is_file() {
            let entry_path = entry.path();
            let mod_path = entry_path.strip_prefix(BACKUP_PATH)?;
            file_list.push(mod_path.to_owned());
        }
        // We shouldn't find any symbolic links or other unusual things
        // in our backup directory:
        else {
            return Err(format_err!("{} isn't a file or a directory", entry.path().to_string_lossy()));
        }
    }
    Ok(())
}

fn collect_unknown_files(mod_file_paths: Vec<PathBuf>, p: &Profile) -> Vec<PathBuf> {
    let mut ret = Vec::<PathBuf>::new();

    'outer: for mod_file_path in mod_file_paths {
        for (_mod_name, manifest) in &p.mods {
            if manifest.files.contains_key(&mod_file_path) {
                continue 'outer;
            }
        }
        // mod_file_path wasn't found in any mods in the profile.
        ret.push(mod_file_path);
    }
    ret
}

/// Verifies integrity of backup files,
/// and returns false if any fail their check.
fn verify_backups(p: &Profile) -> Fallible<bool> {
    let mut ret = true;

    for (_mod_name, manifest) in &p.mods {
        for (mod_path, metadata) in &manifest.files {
            // If there was no backup, there's nothing to check.
            if metadata.original_hash.is_none() { continue; }
            let original_hash = &metadata.original_hash.unwrap();

            let backup_path = mod_path_to_backup_path(&**mod_path);
            let f = File::open(&backup_path).map_err(|e|
                e.context(format!("Couldn't open {}", backup_path.to_string_lossy()))
            )?;
            trace!("Hashing {}", backup_path.to_string_lossy());
            let backup_hash = hash_contents(&mut BufReader::new(f))?;
            if backup_hash != *original_hash {
                debug!("{} hashed to\n{:x},\nexpected {:x}", backup_path.to_string_lossy(),
                    backup_hash.bytes, original_hash.bytes);
                warn!("The backup of {} has changed!\n\
                    Please repair your game files, then run `modman update` \
                    to make new backups.", mod_path.to_string_lossy());
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

    for (_mod_name, manifest) in &p.mods {
        for (mod_path, metadata) in &manifest.files {

            let game_path = mod_path_to_game_path(&**mod_path, p);
            let f = File::open(&game_path).map_err(|e|
                e.context(format!("Couldn't open {}", game_path.to_string_lossy()))
            )?;
            trace!("Hashing {}", game_path.to_string_lossy());
            let game_hash = hash_contents(&mut BufReader::new(f))?;
            if game_hash != metadata.mod_hash {
                debug!("{} hashed to\n{:x},\nexpected {:x}", game_path.to_string_lossy(),
                    game_hash.bytes, metadata.mod_hash.bytes);
                warn!("{} has changed!\n\
                    If the game has been updated, run `modman update` \
                    to update backups and reinstall needed files.",
                    game_path.to_string_lossy());
                ret = false;
            }
        }
    }

    Ok(ret)
}
