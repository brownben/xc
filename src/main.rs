#![warn(clippy::pedantic)]
#![deny(unsafe_code)]

mod discovery;
mod print;
mod python;
mod run;

use clap::Parser;
use rayon::prelude::*;

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

  print::heading(&python::version());

  let tests = discovery::find_tests(&args.paths);
  print::discovery(&tests);

  let _interpreter = python::Interpreter::initialize();
  let progress_bar = print::create_progress_bar(tests.tests.len());

  let results: run::TestSummary = tests
    .tests
    .par_iter()
    .map(|test| python::SubInterpreter::new().run(|| run::test(test)))
    .map(|outcome| {
      progress_bar.suspend(|| print::test_result(&outcome).unwrap());
      progress_bar.inc(1);

      outcome
    })
    .collect();

  progress_bar.finish_and_clear();

  print::summary(&results);
  for result in results.tests {
    if result.is_fail() {
      print::error(&result);
    }
  }
}
