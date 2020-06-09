use std::default::Default;
use std::fs;
use std::io::prelude::*;
use std::path::PathBuf;

use anyhow::*;
use log::*;
use structopt::*;

use crate::profile::*;

/// Create a new mod directory here (or wherever -C gave)
#[derive(Debug, StructOpt)]
pub struct Args {
    /// The root directory where mod files will be installed
    #[structopt(long)]
    root: PathBuf,
}

pub fn run(args: Args) -> Result<()> {
    debug!("Checking if the given --root exists...");

    let root_path = args.root;
    if !root_path.is_dir() {
        bail!("{} is not an existing directory!", root_path.display());
    }

    debug!("Writing an empty profile file...");

    let p = Profile {
        root_directory: root_path,
        mods: Default::default(),
    };
    create_new_profile_file(&p)?;

    info!("Profile written to {}", PROFILE_PATH);

    if let Some(mkdir_err) = fs::create_dir(STORAGE_PATH).err() {
        if mkdir_err.kind() == std::io::ErrorKind::AlreadyExists {
            // Let's remove the profile file we just created so that
            // the user doesn't get an error that it exists next time.
            fs::remove_file(PROFILE_PATH).context(
                "Failed to remove profile file after discovering a backup directory already exists.")?;
            bail!(
                "A backup directory ({}/) already exists.\n\
                 Please move or remove it, then run modman init again.",
                STORAGE_PATH
            );
        } else {
            return Err(Error::from(mkdir_err));
        }
    }

    fs::create_dir(TEMPDIR_PATH).context("Couldn't create temporary storage directory ({}/)")?;
    fs::create_dir(BACKUP_PATH).context("Couldn't create backup directory ({}/)")?;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(BACKUP_README)?
        .write_all(
            format!(
                r#"modman backs up the game files here.

{0}/ holds partial copies of game files as we back them up.
Once we've finished copying them, they are moved to {1}/.
This ensures that {1}/ only contains complete backups.

If modman is closed while performing a backup, some leftover files
might be found in {0}/.
Feel free to delete them."#,
                TEMPDIR_PATH, BACKUP_PATH
            )
            .as_bytes(),
        )
        .with_context(|| format!("Couldn't create backup README ({})", BACKUP_README))?;

    info!("Backup directory ({}/) created", STORAGE_PATH);

    Ok(())
}
