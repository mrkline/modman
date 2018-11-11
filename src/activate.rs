use std::fs::File;
use std::path::Path;
use std::process::exit;

use failure::*;

use crate::modification::*;
use crate::profile::*;

static USAGE: &str = r#"
Usage: modman activate [options] <MOD>

Activate a mod at the path <MOD>.
Mods can be in two formats:
  1. A directory containing a VERSION.txt file, a README.txt file,
     and a single directory, which will be treated as the root of the mod files.
  2. A .zip archive containing the same.
"#;

fn print_usage() -> ! {
    println!("{}", USAGE);
    exit(0);
}

fn eprint_usage() -> ! {
    eprintln!("{}", USAGE);
    exit(2);
}

pub fn activate_command(args: &[String], verbosity: u8) -> Result<(), Error> {
    if args.len() == 1 && args[0] == "help" {
        print_usage();
    }

    if args.is_empty() {
        eprint_usage();
    }

    if verbosity > 0 {
        eprintln!("Loading profile...");
    }

    let f = File::open(PROFILE_PATH).map_err(|e| {
        let ctxt = format!("Couldn't open profile file ({})", PROFILE_PATH);
        e.context(ctxt)
    })?;

    let p: Profile = serde_json::from_reader(f).context("Couldn't parse profile file")?;

    for mod_name in args {
        if verbosity > 0 {
            eprintln!("Activating {}...", mod_name);
        }

        let mod_path = Path::new(mod_name);

        // First sanity check: we haven't already added this mod.
        if p.mods.contains_key(mod_path) {
            return Err(format_err!("{} has already been activated!", mod_name));
        }

        // Open it up!
        let mut m = open_mod(mod_path)?;

        // Next, look at all the paths we currently have,
        // and make sure the new file doesn't contain any of them.
        for new_mod_path in m.paths() {
            if verbosity > 2 {
                eprintln!(
                    "Checking {} for conflicts...",
                    new_mod_path.to_string_lossy()
                );
            }
            for (active_mod_name, active_mod) in &p.mods {
                if active_mod.files.contains_key(&new_mod_path) {
                    return Err(format_err!(
                        "{} would overwrite a file from {}",
                        new_mod_path.to_string_lossy(),
                        active_mod_name.to_string_lossy()
                    ));
                }
            }
        }
    }

    Ok(())
}
