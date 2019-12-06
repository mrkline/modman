use std::env;
use std::process::exit;

use atty::*;
use failure::*;
use getopts::{Options, ParsingStyle};
use log::*;

mod activate;
mod check;
mod deactivate;
mod dir_mod;
mod encoding;
mod error;
mod file_utils;
mod hash_serde;
mod init;
mod journal;
mod list;
mod modification;
mod profile;
mod repair;
mod update;
mod usage;
mod version_serde;
mod zip_mod;

use crate::activate::*;
use crate::check::*;
use crate::deactivate::*;
use crate::init::*;
use crate::list::*;
use crate::repair::*;
use crate::update::*;
use crate::usage::*;

static USAGE: &str = r#"Usage: modman [options] <command> [command options]

<command> is one of:

  init: Create a new mod configuration in this directory.

  add/activate: Activate a mod package, backing up files it overwrite.

  remove/deactivate: Deactivate a mod package, restoring files it overwrote.

  list: List currently-activated mod packages,
        or the files they've overwritten.

  update: Following an update, discover which modded files were updated.
          Backup those updates, then overwrite them with the mod files again.

  check: Verifies that active mod files and backups are still good.

  help: Print this information.
"#;

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
            eprint_usage(USAGE, &opts);
        }
    };

    let verbosity = matches.opt_count("v");
    let mut errlog = stderrlog::new();
    // The +1 is because we want -v to give info, not warn.
    errlog.verbosity(verbosity + 1);
    if atty::is(Stream::Stdout) {
        errlog.color(stderrlog::ColorChoice::Auto);
    } else {
        errlog.color(stderrlog::ColorChoice::Never);
    }
    errlog.init()?;

    if let Some(chto) = matches.opt_str("C") {
        env::set_current_dir(&chto)
            .with_context(|_| format!("Couldn't set working directory to {}", chto))?;
    }

    let mut free_args = matches.free;

    if free_args.is_empty() {
        eprintln!("Please give a command.");
        eprint_usage(USAGE, &opts);
    }

    // If the user passed multiple args (see above),
    // and the first one is "help", swap it with the second so that
    // "help init" produces the same help text as "init help".
    if free_args.len() > 1 && free_args[0] == "help" {
        free_args.swap(0, 1);
    }

    match free_args[0].as_ref() {
        "add"|"activate" => activate_command(&free_args[1..]),
        "remove"|"deactivate" => deactivate_command(&free_args[1..]),
        "check" => check_command(&free_args[1..]),
        "help" => print_usage(USAGE, &opts),
        "init" => init_command(&free_args[1..]),
        "list" => list_command(&free_args[1..]),
        "update" => update_command(&free_args[1..]),
        "repair" => repair_command(&free_args[1..]),
        wut => {
            eprintln!("Unknown command: {}", wut);
            eprint_usage(USAGE, &opts);
        }
    }
}

fn main() {
    do_it().unwrap_or_else(|e| {
        error!("{}", crate::error::pretty_error(&e));
        exit(1);
    });
}
