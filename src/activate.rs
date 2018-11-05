use failure::*;

use std::fs::{metadata, File};
use std::path::Path;
use std::process::exit;

use crate::profile::*;

static USAGE: &str = r#"
Usage: modman [-C <DIR>] activate [options] <MOD>

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

pub fn activate_command(args: &[String]) -> Result<(), Error> {
    if args.len() == 1 && args[0] == "help" {
        print_usage();
    }

    if args.is_empty() {
        eprint_usage();
    }

    let f = File::open(PROFILE_PATH).map_err(|e| {
        let ctxt = format!("Couldn't open profile file ({})", PROFILE_PATH);
        e.context(ctxt)
    })?;
    let p: Profile = serde_json::from_reader(f).context("Couldn't parse profile file")?;

    for mod_name in args {
        let mod_path = Path::new(mod_name);

        // Alright, let's stat the thing:
        let stat = metadata(mod_path).map_err(|e| {
            let ctxt = format!("Couldn't find {}", mod_name);
            e.context(ctxt)
        })?;

        // First sanity check: we haven't already added this mod.
        if p.mods.contains_key(mod_path) {
            return Err(format_err!("{} has already been activated!", mod_name));
        }
    }

    Ok(())
}
