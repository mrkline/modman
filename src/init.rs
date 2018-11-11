use std::default::Default;
use std::fs::File;
use std::path::PathBuf;
use std::process::exit;

use failure::*;
use getopts::Options;

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

pub fn init_command(args: &[String], verbosity: u8) -> Result<(), Error> {
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

    if verbosity > 0 {
        eprintln!("Checking for an existing profile file...");
    }

    if profile_exists() {
        if matches.opt_present("f") {
            eprintln!("One does, but --force was given.");
        } else {
            return Err(format_err!(
                "Profile file ({}) already exists!",
                PROFILE_PATH
            ));
        }
    }

    if verbosity > 0 {
        eprintln!("Checking if the given --root exists...");
    }

    let root_path = PathBuf::from(&matches.opt_str("root").unwrap());
    if !root_path.is_dir() {
        return Err(format_err!(
            "{} is not an existing directory!",
            root_path.to_string_lossy()
        ));
    }

    if verbosity > 0 {
        eprintln!("Writing an empty profile file...");
    }

    let p = Profile {
        root_directory: root_path,
        mods: Default::default(),
    };

    let f = File::create(PROFILE_PATH)?;
    serde_json::to_writer_pretty(f, &p)?;

    eprintln!("Profile file written to {}", PROFILE_PATH);

    Ok(())
}
