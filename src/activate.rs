use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::exit;

use failure::*;
use log::*;
use sha2::*;
use tempfile::tempfile;

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

pub fn activate_command(args: &[String]) -> Fallible<()> {
    if args.len() == 1 && args[0] == "help" {
        print_usage();
    }

    if args.is_empty() {
        eprint_usage();
    }

    info!("Loading profile...");

    let f = File::open(PROFILE_PATH).map_err(|e| {
        let ctxt = format!("Couldn't open profile file ({})", PROFILE_PATH);
        e.context(ctxt)
    })?;

    let p: Profile = serde_json::from_reader(f).context("Couldn't parse profile file")?;

    for mod_name in args {
        info!("Activating {}...", mod_name);

        let mod_path = Path::new(mod_name);

        // First sanity check: we haven't already added this mod.
        if p.mods.contains_key(mod_path) {
            return Err(format_err!("{} has already been activated!", mod_name));
        }

        // Open it up!
        let mut m = open_mod(mod_path)?;

        let mod_file_paths = m.paths()?;

        // Next, look at all the paths we currently have,
        // and make sure the new file doesn't contain any of them.
        check_for_profile_conflicts(&mod_file_paths, &p)?;

        for mod_file_path in &mod_file_paths {
            // TEST
            let mut br = BufReader::new(m.read_file(mod_file_path)?);
            let hnt = hash_and_temp(&mut br)?;
            debug!("{:x}: {}", hnt.hash, mod_file_path.to_string_lossy());
        }
    }

    Ok(())
}

fn check_for_profile_conflicts(mod_file_paths: &Vec<PathBuf>, p: &Profile) -> Fallible<()> {
    for mod_file_path in mod_file_paths {
        for (active_mod_name, active_mod) in &p.mods {
            if active_mod.files.contains_key(mod_file_path.as_path()) {
                return Err(format_err!(
                    "{} would overwrite a file from {}",
                    mod_file_path.to_string_lossy(),
                    active_mod_name.to_string_lossy()
                ));
            }
        }
    }
    Ok(())
}

struct HashAndTempCopy {
    pub hash: FileHash,
    pub temp_copy: File,
}

fn hash_and_temp<R: BufRead>(mut reader: R) -> Fallible<HashAndTempCopy> {
    let mut temp_copy = tempfile().context("Couldn't create a temporary file")?;

    let mut hasher = Sha256::new();

    loop {
        let slice_length = {
            let slice = reader.fill_buf()?;
            if slice.is_empty() {
                break;
            }
            temp_copy.write(slice)?;
            hasher.input(slice);
            slice.len()
        };
        reader.consume(slice_length);
    }

    Ok(HashAndTempCopy {
        hash: hasher.result(),
        temp_copy,
    })
}
