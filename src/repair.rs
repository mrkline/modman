use std::fs;
use std::path::Path;

use anyhow::*;
use log::*;
use structopt::*;

use crate::journal::*;
use crate::profile::*;

/// Tries to return things to how they were if `add` was interrupted
///
/// While installing a mod, `modman add` keeps a journal of files it's adding
/// and replacing in the game directory. If it's interrupted before it can finish,
/// we can use the journal to try to undo the partial installation, restoring the
/// game files to their previous state.
#[derive(Debug, StructOpt)]
#[structopt(verbatim_doc_comment)]
pub struct Args {
    #[structopt(short = "n", long)]
    dry_run: bool,
}

pub fn run(args: Args) -> Result<()> {
    let p = load_and_check_profile()?;

    let journal_map = read_journal()?;

    if journal_map.is_empty() {
        info!("Activation joural is empty or doesn't exist - nothing to repair.");
        return Ok(());
    }
    // We'll make most messages INFO level here, since
    // someone is having a bad time if they're running this.
    // We'd like to be verbose to help them figure out what the situation is.
    info!("Found a journal from an interrupted `modman add`.");
    info!("Restoring what files we can find...");

    let mut clean_run = true;
    for (path, action) in &journal_map {
        match try_to_undo(path, *action, &p, args.dry_run) {
            Ok(()) => (),
            Err(e) => {
                error!("{:#}", e);
                clean_run = false;
            }
        }
    }

    if clean_run {
        if !args.dry_run {
            info!(
                "Repair complete, removing journal file. \
                 Game files should be as they were before the interrupted `modman add`."
            );
            fs::remove_file(get_journal_path()).context("Couldn't delete activation journal")?;
        }
    } else {
        bail!(
            "Errors encountered while undoing the interrupted `modman add`. \
             Leaving the journal file around; good luck and godspeed."
        );
    }

    Ok(())
}

fn try_to_undo(path: &Path, action: JournalAction, p: &Profile, dry_run: bool) -> Result<()> {
    if p.mods
        .values()
        .any(|manifest| manifest.files.keys().any(|file| file == path))
    {
        bail!(
            "{} is referenced in both the activation jurnal and the profile. \
        Something is wrong - journals should be deleted before their mod is added to the profile.",
            path.display()
        );
    }

    match action {
        JournalAction::Added => try_to_remove(path, &p, dry_run),
        JournalAction::Replaced => try_to_restore(path, &p, dry_run),
    }
}

fn try_to_remove(path: &Path, p: &Profile, dry_run: bool) -> Result<()> {
    info!("Remove {}", path.display());
    if !dry_run {
        let game_path = mod_path_to_game_path(path, &p.root_directory);
        fs::remove_file(&game_path)
            .with_context(|| format!("Couldn't remove {}", game_path.display()))?;
    }

    Ok(())
}

fn try_to_restore(path: &Path, p: &Profile, dry_run: bool) -> Result<()> {
    info!("Restore {}", path.display());
    if !dry_run {
        let backup_path = mod_path_to_backup_path(path);
        let game_path = mod_path_to_game_path(path, &p.root_directory);
        // Let copy fail if the backup doesn't exist.
        fs::copy(&backup_path, &game_path).with_context(|| {
            format!(
                "Couldn't copy {} to {}",
                backup_path.display(),
                game_path.display()
            )
        })?;
        // If restoration succeeds, let's remove the backup.
        fs::remove_file(&backup_path)
            .with_context(|| format!("Couldn't remove {}", backup_path.display()))?;
    }

    Ok(())
}
