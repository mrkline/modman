use std::default::Default;
use std::fs::*;
use std::io::prelude::*;
use std::path::PathBuf;
use std::process::exit;

use failure::*;
use getopts::Options;
use log::*;

use crate::profile::*;

static USAGE: &str = r#"Usage: modman init [options]

Create a new mod configuration file in this directory (or the one given with -C).
The file will be named"#;

fn print_usage(opts: &Options) -> ! {
    let help = format!("{} {}", USAGE, PROFILE_PATH);
    println!("{}", opts.usage(&help));
    exit(0);
}

fn eprint_usage(opts: &Options) -> ! {
    let help = format!("{} {}", USAGE, PROFILE_PATH);
    eprintln!("{}", opts.usage(&help));
    exit(2);
}

pub fn init_command(args: &[String]) -> Fallible<()> {
    let mut opts = Options::new();
    opts.reqopt(
        "",
        "root",
        "The root directory (usually a game's directory) where mods should be installed.",
        "<DIR>",
    );

    if args.len() == 1 && args[0] == "help" {
        print_usage(&opts);
    }

    let matches = match opts.parse(args) {
        Ok(m) => m,
        Err(f) => {
            eprintln!("{}", f.to_string());
            eprint_usage(&opts);
        }
    };

    let free_args = &matches.free;

    if !free_args.is_empty() {
        eprint_usage(&opts);
    }

    debug!("Checking if the given --root exists...");

    let root_path = PathBuf::from(&matches.opt_str("root").unwrap());
    if !root_path.is_dir() {
        return Err(format_err!(
            "{} is not an existing directory!",
            root_path.to_string_lossy()
        ));
    }

    debug!("Writing an empty profile file...");

    let p = Profile {
        root_directory: root_path,
        mods: Default::default(),
    };

    let f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(PROFILE_PATH)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                format_err!("A profile already exists.")
            } else {
                failure::Error::from(e)
            }
        })?;
    serde_json::to_writer_pretty(f, &p)?;
    drop(p);

    info!("Profile written to {}", PROFILE_PATH);

    if let Some(mkdir_err) = create_dir(STORAGE_PATH).err() {
        if mkdir_err.kind() == std::io::ErrorKind::AlreadyExists {
            // Let's remove the profile file we just created so that
            // the user doesn't get an error that it exists next time.
            remove_file(PROFILE_PATH).context(
                "Failed to remove profile file after discovering a backup directory already exists.")?;
            return Err(format_err!(
                "A backup directory ({}/) already exists.\n\
                 Please move or remove it, then run modman init again.",
                STORAGE_PATH
            ));
        } else {
            return Err(failure::Error::from(mkdir_err));
        }
    }

    create_dir(TEMPDIR_PATH).context("Couldn't create temporary storage directory ({}/)")?;
    create_dir(BACKUP_PATH).context("Couldn't create backup directory ({}/)")?;
    OpenOptions::new()
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
        )?;

    info!("Backup directory ({}/) created", STORAGE_PATH);

    Ok(())
}
