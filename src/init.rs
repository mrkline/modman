use std::default::Default;
use std::fs::*;
use std::path::PathBuf;
use std::process::exit;

use failure::*;
use getopts::Options;
use log::*;

use crate::profile::*;

static USAGE: &str = r#"
Usage: modman init [options]

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
    opts.optflag(
        "f",
        "force",
        "Recreate the mod configuration file, even if one already exists.",
    );
    opts.reqopt(
        "",
        "root",
        "The root directory (usually a game's directory) where mods should be installed.",
        "<DIR>",
    );

    let matches = match opts.parse(args) {
        Ok(m) => m,
        Err(f) => {
            eprintln!("{}", f.to_string());
            eprint_usage(&opts);
        }
    };

    let free_args = &matches.free;

    if free_args.len() == 1 && free_args[0] == "help" {
        print_usage(&opts);
    }

    if !free_args.is_empty() {
        eprint_usage(&opts);
    }

    info!("Checking if the given --root exists...");

    let root_path = PathBuf::from(&matches.opt_str("root").unwrap());
    if !root_path.is_dir() {
        return Err(format_err!(
            "{} is not an existing directory!",
            root_path.to_string_lossy()
        ));
    }

    info!("Writing an empty profile file...");

    let p = Profile {
        root_directory: root_path,
        mods: Default::default(),
    };

    let mut open_opts = OpenOptions::new();
    open_opts.write(true);
    // Only allow the file to be overwritten if --force was given.
    if matches.opt_present("f") {
        open_opts.create(true);
    } else {
        open_opts.create_new(true);
    }

    let f = open_opts.open(PROFILE_PATH).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            format_err!("A profile file already exists (use --force to overwrite).")
        } else {
            failure::Error::from(e)
        }
    })?;
    serde_json::to_writer_pretty(f, &p)?;

    eprintln!("Profile file written to {}", PROFILE_PATH);

    Ok(())
}
