use getopts::{Options, ParsingStyle};
use std::env;
use std::process::exit;

mod init;
mod profile;
mod version_serde;

use crate::init::*;

static USAGE: &str = r#"
Usage: modman [options] <command> [command options]

<command> is one of:

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

fn print_usage(opts: &Options) -> ! {
    println!("{}", opts.usage(USAGE));
    exit(0);
}

fn eprint_usage(opts: &Options) -> ! {
    eprintln!("{}", opts.usage(USAGE));
    exit(1);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    let mut opts = Options::new();
    opts.optopt(
        "C",
        "directory",
        "run modman as if it were started in <DIR> instead of the current directory.",
        "<DIR>",
    );
    // We don't want to eat the subcommands' args.
    opts.parsing_style(ParsingStyle::StopAtFirstFree);

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            eprintln!("{}", f.to_string());
            eprint_usage(&opts);
        }
    };

    if matches.free.len() == 1 && matches.free[0] == "help" {
        print_usage(&opts);
    }

    if let Some(chto) = matches.opt_str("C") {
        if !env::set_current_dir(&chto).is_ok() {
            eprintln!("Couldn't set working directory to {}", chto);
            exit(1);
        }
    }

    let mut free_args = matches.free;

    if free_args.is_empty() {
        eprintln!("Please give a command.");
        eprint_usage(&opts);
    }

    // If the user passed multiple args (see above),
    // and the first one is "help", swap it with the second so that
    // "help init" produces the same help text as "init help".
    if free_args[0] == "help" {
        free_args.swap(0, 1);
    }

    match free_args[0].as_ref() {
        "init" => init_command(&free_args[1..]),
        wut => {
            eprintln!("Unknown command: {}", wut);
            eprint_usage(&opts);
        }
    }
}
