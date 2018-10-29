use getopts::Options;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::process::*;
use zip::read::*;

mod profile;
mod version_serde;

static USAGE: &str = r#"
Usage: modman [options] <verb> [verb options]

<verb> is one of:

  init: Create a new mod configuration in this directory.

  activate: Activate a mod package, backing up the game files they overwrite.

  deactivate: Deactivate a mod package, restoring the game files they overwrote.

  list: List currently-activated mod packages,
        or the game files they've overwritten.

  update: Following an update, discover which modded game files were updated.
          Backup those updates, then overwrite them with the mod files again.

  verify: Verifies that the backup files are still good,
          the active mod files are still good,
          or both.
"#;

fn print_usage(opts: &Options, code: i32) {
    println!("{}", opts.usage(USAGE));
    exit(code);
}

fn eprint_usage(opts: &Options, code: i32) {
    eprintln!("{}", opts.usage(USAGE));
    exit(code);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    let mut opts = Options::new();
    opts.optflag("h", "help", "Print this help menu");
    opts.optopt(
        "C",
        "directory",
        "run modman as if it were started in <DIR> instead of the current directory.",
        "<DIR>",
    );

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => panic!(f),
    };

    if matches.opt_present("h") || (matches.free.len() == 1 && matches.free[0] == "help") {
        print_usage(&opts, 0);
    }

    if let Some(chto) = matches.opt_str("C") {
        if !env::set_current_dir(&chto).is_ok() {
            eprintln!("Couldn't set working directory to {}", chto);
            exit(1);
        }
    }

    let input = &matches.free;
    match input.len() {
        0 => {
            eprintln!("No input file provided");
            eprint_usage(&opts, 1)
        }
        1 => (),
        _ => {
            eprintln!("Can only take one input file at once");
            eprint_usage(&opts, 1)
        }
    };

    let f = File::open(&input[0])?;
    let mut zip = ZipArchive::new(f)?;

    for i in 0..zip.len() {
        let file = zip.by_index(i).unwrap();
        println!("Filename: {}", file.name());
    }

    println!("Hello, world!");
    Ok(())
}
