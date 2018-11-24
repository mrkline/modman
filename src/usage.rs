use std::process::exit;

use getopts::Options;

pub fn print_usage(usage: &str, opts: &Options) -> ! {
    println!("{}", opts.usage(usage));
    exit(0);
}

pub fn eprint_usage(usage: &str, opts: &Options) -> ! {
    eprintln!("{}", opts.usage(usage));
    exit(2);
}

