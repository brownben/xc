#![warn(clippy::pedantic)]
#![deny(unsafe_code)]

mod discovery;
mod json;
mod print;
mod python;
mod run;

use clap::Parser;
use rayon::prelude::*;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
  /// List of files or directories to test [default: .]
  #[clap(default_value = ".")]
  pub paths: Vec<std::path::PathBuf>,

  /// Output results as JSON to stdout
  #[clap(long, default_value_t = false)]
  pub json: bool,
}

fn main() {
  let args = Args::parse();

  print::heading(&python::version());

  let discovered = discovery::find_tests(&args.paths);
  print::discovery(&discovered);

  let _interpreter = python::Interpreter::initialize();
  let progress_bar = print::create_progress_bar(discovered.tests.len());

  let results: run::TestSummary = discovered
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

  print::results_summary(&results);

  if args.json {
    print::json_results(&results);
  }
}
