use failure::*;

use crate::profile::*;
use crate::usage::*;

static USAGE: &str = r#"Usage: modman list [options]

List installed mods and (optionally) their files.
"#;

pub fn list_command(args: &[String]) -> Fallible<()> {
    let mut opts = getopts::Options::new();
    opts.optflag("f", "files", "List the files installed by each mod.");

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

    let p = load_and_check_profile()?;

    for (mod_name, mod_manifest) in p.mods {
        println!("{}", mod_name.to_string_lossy());
        if print_files {
            for f in mod_manifest.files.keys() {
                println!("\t{}", f.to_string_lossy());
            }
        }
    }

    Ok(())
}
