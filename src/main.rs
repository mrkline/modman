use std::path::PathBuf;

use anyhow::*;
use atty::*;
use structopt::*;

mod add;
mod check;
mod dir_mod;
mod encoding;
mod file_utils;
mod hash_serde;
mod init;
mod journal;
mod list;
mod modification;
mod profile;
mod remove;
mod repair;
mod update;
mod version_serde;
mod zip_mod;

/// An OVGME-like mod manager with exciting 21st century tech - like threads!
#[derive(Debug, StructOpt)]
struct Options {
    /// Print progress to stderr. Pass multiple times for more verbosity (info, debug, trace)
    #[structopt(short, long, parse(from_occurrences))]
    verbosity: usize,

    /// Do everything with <DIR> as the working directory.
    #[structopt(short = "C", long, name = "DIR")]
    directory: Option<PathBuf>,

    #[structopt(subcommand)]
    subcommand: Subcommand,
}

#[derive(Debug, StructOpt)]
enum Subcommand {
    Init(init::Args),
    Add(add::Args),
    Remove(remove::Args),
    List(list::Args),
    /// Check for possible problems with installed mods and backed up files.
    Check,
    Update(update::Args),
    Repair(repair::Args),
}

fn main() -> Result<()> {
    let args = Options::from_args();

    let mut errlog = stderrlog::new();
    // The +1 is because we want -v to give info, not warn.
    errlog.verbosity(args.verbosity + 1);
    if atty::is(Stream::Stdout) {
        errlog.color(stderrlog::ColorChoice::Auto);
    } else {
        errlog.color(stderrlog::ColorChoice::Never);
    }
    errlog.init()?;

    if let Some(chto) = args.directory {
        std::env::set_current_dir(&chto)
            .with_context(|| format!("Couldn't set working directory to {}", chto.display()))?;
    }

    match args.subcommand {
        Subcommand::Init(i) => init::run(i),
        Subcommand::Add(a) => add::run(a),
        Subcommand::Remove(r) => remove::run(r),
        Subcommand::List(l) => list::run(l),
        Subcommand::Check => check::run(),
        Subcommand::Update(u) => update::run(u),
        Subcommand::Repair(r) => repair::run(r),
    }
}
