#![warn(clippy::pedantic)]
#![deny(unsafe_code)]
#![allow(unused)]

mod discovery;
mod print;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
  /// List of files or directories to check for tests.
  #[clap(
    help = "List of files or directories to test [default: .]",
    default_value = "."
  )]
  pub paths: Vec<std::path::PathBuf>,
}

fn main() {
  let args = Args::parse();

  print::heading();

  let tests = discovery::find_tests(&args.paths);
  print::discovery(&tests);
}
