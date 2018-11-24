use std::env;
use std::process::exit;

use failure::*;
use getopts::{Options, ParsingStyle};

mod activate;
mod init;
mod modification;
mod profile;
mod version_serde;
mod zip_mod;

use crate::activate::*;
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
    exit(2);
}

fn do_it() -> Fallible<()> {
    let args: Vec<String> = env::args().collect();

    let mut opts = Options::new();
    opts.optopt(
        "C",
        "directory",
        "run modman as if it were started in <DIR> instead of the current directory.",
        "<DIR>",
    );
    opts.optflagmulti(
        "v",
        "verbose",
        "print progress to stderr. Pass multiple times for more info.",
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

    let verbosity = matches.opt_count("v");
    // The +1 is because we want -v to give info, not warn.
    stderrlog::new().verbosity(verbosity + 1).init()?;

    if let Some(chto) = matches.opt_str("C") {
        env::set_current_dir(&chto).map_err(|e| {
            e.context(format!("Couldn't set working directory to {}", chto))
        })?;
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
        "activate" => activate_command(&free_args[1..]),
        wut => {
            eprintln!("Unknown command: {}", wut);
            eprint_usage(&opts);
        }
    }
}

fn main() {
    do_it().unwrap_or_else(|e| {
        eprintln!("{}", pretty_error(&e));
        exit(1);
    });
}

// Borrowed lovingly from Burntsushi:
// https://www.reddit.com/r/rust/comments/8fecqy/can_someone_show_an_example_of_failure_crate_usage/dy2u9q6/
// Chains errors into a big string.
fn pretty_error(err: &failure::Error) -> String {
    let mut pretty = err.to_string();
    let mut prev = err.as_fail();
    while let Some(next) = prev.cause() {
        pretty.push_str(":\n");
        pretty.push_str(&next.to_string());
        if let Some(bt) = next.backtrace() {
            let mut bts = bt.to_string();
            // If RUST_BACKTRACE is not defined, next.backtrace() gives us
            // Some(bt), but bt.to_string() gives us an empty string.
            // If we push a newline to the return value and nothing else,
            // we get something like:
            // ```
            // Some errror
            // :
            // Its cause
            // ```
            if !bts.is_empty() {
                bts.push_str("\n");
                pretty.push_str(&bts);
            }
        }
        prev = next;
    }
    pretty
}
