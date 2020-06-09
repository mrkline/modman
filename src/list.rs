use anyhow::*;
use log::*;
use structopt::*;

use crate::modification::*;
use crate::profile::*;

/// List installed mods.
#[derive(Debug, StructOpt)]
pub struct Args {
    /// List the files installed by each mod.
    #[structopt(short, long)]
    files: bool,

    /// Print each mod's README
    #[structopt(short, long)]
    readme: bool,
}

pub fn run(args: Args) -> Result<()> {
    let p = load_and_check_profile()?;

    for (mod_name, mod_manifest) in p.mods {
        println!("{} (v{})", mod_name.display(), mod_manifest.version);
        if args.readme {
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
                Err(e) => warn!("Couldn't open mod {}:\n{:#}", mod_name.display(), e),
            }
        }
        if args.files {
            for f in mod_manifest.files.keys() {
                println!("\t{}", f.display());
            }
        }
    }

    Ok(())
}
