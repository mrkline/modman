use failure::*;
use log::*;

use crate::error::*;
use crate::modification::*;
use crate::profile::*;
use crate::usage::*;

static USAGE: &str = r#"Usage: modman list [options]

List installed mods and (optionally) their files.
"#;

pub fn list_command(args: &[String]) -> Fallible<()> {
    let mut opts = getopts::Options::new();
    opts.optflag("f", "files", "List the files installed by each mod.");
    opts.optflag("r", "readme", "Print each mod's README under its name");

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

    let print_files = matches.opt_present("f");
    let print_readme = matches.opt_present("r");

    let p = load_and_check_profile()?;

    for (mod_name, mod_manifest) in p.mods {
        println!("{} (v{})", mod_name.display(), mod_manifest.version);
        if print_readme {
            // We don't store READMEs in the manifest, so go get the mod itself.
            match open_mod(&mod_name) {
                Ok(m) => {
                    let opened_version = m.version();
                    if opened_version != &mod_manifest.version {
                        warn!("Mod file has a different version ({}) than the one that was installed ({})",
                              opened_version, mod_manifest.version);
                    }
                    println!("{}", m.readme());
                }
                Err(e) => warn!(
                    "Couldn't open mod {}:\n{}",
                    mod_name.display(),
                    pretty_error(&e)
                ),
            }
        }
        if print_files {
            for f in mod_manifest.files.keys() {
                println!("\t{}", f.display());
            }
        }
    }

    Ok(())
}
