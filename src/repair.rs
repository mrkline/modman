use std::fs::*;
use std::path::Path;

use failure::*;
use log::*;

use crate::journal::*;
use crate::profile::*;
use crate::usage::*;

static USAGE: &str = r#"Usage: modman repair

Tries to return things to how they were if `modman activate` was interrupted.

While installing a mod, `modman activate` keeps a journal of files it's adding
and replacing in the game directory. If it's interrupted before it can finish,
we can use the journal to try to undo the partial installation, restoring the
game files to their previous state.
"#;

pub fn repair_command(args: &[String]) -> Fallible<()> {
    let mut opts = getopts::Options::new();
    opts.optflag(
        "n",
        "dry-run",
        "See what actions `modman repair` would take, but don't change any files.",
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

    let dry_run = matches.opt_present("n");

    let p = load_and_check_profile()?;

    let journal_map = read_journal()?;

    if journal_map.is_empty() {
        info!("Activation joural is empty or doesn't exist - nothing to repair.");
        return Ok(());
    }
    // We'll make most messages INFO level here, since
    // someone is having a bad time if they're running this.
    // We'd like to be verbose to help them figure out what the situation is.
    info!("Found a journal from an interrupted `modman activate`.");
    info!("Restoring what files we can find...");

    let mut clean_run = true;
    for (path, action) in &journal_map {
        match try_to_undo(path, *action, &p, dry_run) {
            Ok(()) => (),
            Err(e) => {
                error!("{}", crate::error::pretty_error(&e));
                clean_run = false;
            }
        }
    }

    if clean_run {
        info!(
            "Repair complete, removing journal file. \
             Game files should be as they were before the interrupted `modman activate`."
        );
        remove_file(get_journal_path())
            .map_err(|e| Error::from(e.context("Couldn't delete activation journal")))?;
    } else {
        bail!(
            "Errors encountered while undoing the interrupted `modman activate`. \
             Leaving the journal file around; good luck and godspeed."
        );
    }

    Ok(())
}

fn try_to_undo(path: &Path, action: JournalAction, p: &Profile, dry_run: bool) -> Fallible<()> {
    if p.mods
        .values()
        .any(|manifest| manifest.files.keys().any(|file| file == path))
    {
        bail!("{} is referenced in both the activation jurnal and the profile. \
        Something is wrong - journals should be deleted before their mod is added to the profile.",
        path.to_string_lossy());
    }

    match action {
        JournalAction::Added => try_to_remove(path, &p, dry_run),
        JournalAction::Replaced => try_to_restore(path, &p, dry_run),
    }
}

fn try_to_remove(path: &Path, p: &Profile, dry_run: bool) -> Fallible<()> {
    info!("Remove {}", path.to_string_lossy());
    if !dry_run {
        let game_path = mod_path_to_game_path(path, &p.root_directory);
        remove_file(&game_path).map_err(|e| {
            Error::from(e.context(format!("Couldn't remove {}", game_path.to_string_lossy())))
        })?;
    }

    Ok(())
}

fn try_to_restore(path: &Path, p: &Profile, dry_run: bool) -> Fallible<()> {
    info!("Restore {}", path.to_string_lossy());
    if !dry_run {
        let backup_path = mod_path_to_backup_path(path);
        let game_path = mod_path_to_game_path(path, &p.root_directory);
        // Let copy fail if the backup doesn't exist.
        copy(&backup_path, &game_path).map_err(|e| {
            Error::from(e.context(format!(
                "Couldn't copy {} to {}",
                backup_path.to_string_lossy(),
                game_path.to_string_lossy()
            )))
        })?;
        // If restoration succeeds, let's remove the backup.
        remove_file(&backup_path).map_err(|e| {
            Error::from(e.context(format!("Couldn't remove {}", backup_path.to_string_lossy())))
        })?;
    }

    Ok(())
}
