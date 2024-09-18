#![warn(clippy::pedantic)]
#![allow(clippy::unsafe_derive_deserialize)]
#![deny(unsafe_code)]

mod discovery;
mod json;
mod print;
mod python;
mod run;

#[cfg(test)]
mod tests;

use clap::Parser;
use rayon::prelude::*;
use run::TestOutcome;
use std::{
  process::ExitCode,
  sync::atomic::{AtomicBool, Ordering::Relaxed},
  time::{Duration, Instant},
};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
  /// List of files or directories to test [default: .]
  #[clap(default_value = ".")]
  pub paths: Vec<std::path::PathBuf>,

  /// Don't stop executing tests after one has failed
  #[clap(long, default_value_t = false)]
  pub no_fail_fast: bool,

  /// Output results as JSON to stdout
  #[clap(long, default_value_t = false)]
  pub json: bool,
}

fn main() -> ExitCode {
  let args = Args::parse();
  print::heading(&python::version());

  // Discover tests
  let discovered = discovery::find_tests(&args.paths);
  print::discovery(&discovered);

  // Main Python interpreter must be initialized in the main thread
  let _interpreter = python::Interpreter::initialize();

  // Run tests
  let progress_bar = print::create_progress_bar(discovered.test_count);
  let seen_failed_test = AtomicBool::new(false);
  let mut results: TestSummary = discovered
    .tests
    .par_iter()
    .map(|test| python::SubInterpreter::new().run(|| run::test(test)))
    .map(|result| {
      if args.no_fail_fast {
        return Some(result);
      }

      if seen_failed_test.load(Relaxed) {
        return None;
      }

      if result.is_fail() {
        seen_failed_test.store(true, Relaxed);
      }

      Some(result)
    })
    .while_some()
    .inspect(|result| {
      progress_bar.suspend(|| print::test_result(result).unwrap());
      progress_bar.inc(1);
    })
    .collect();
  progress_bar.finish_and_clear();
  results.set_total_number_of_tests(discovered.test_count);

  // Report results
  print::results_summary(&results);
  if args.json {
    print::json_results(&results);
  };

  if results.failed == 0 && results.passed > 0 {
    ExitCode::SUCCESS
  } else {
    ExitCode::FAILURE
  }
}

/// Summary of all tests that were run
#[derive(Clone, Debug, Default)]
pub struct TestSummary<'tests> {
  pub duration: Duration,

  pub passed: usize,
  pub skipped: usize,
  pub failed: usize,

  pub tests: Vec<TestOutcome<'tests>>,
}
impl TestSummary<'_> {
  #[must_use]
  pub fn run(&self) -> usize {
    self.passed + self.failed
  }

  /// Ensure that the number of skipped tests includes those skipped by fail fast
  pub fn set_total_number_of_tests(&mut self, total: usize) {
    if self.run() + self.skipped < total {
      self.skipped = total - self.run();
    }
  }
}
impl<'tests> FromParallelIterator<TestOutcome<'tests>> for TestSummary<'tests> {
  fn from_par_iter<T: IntoParallelIterator<Item = TestOutcome<'tests>>>(iter: T) -> Self {
    let start_time = Instant::now();
    let tests: Vec<_> = iter.into_par_iter().collect();
    let duration = start_time.elapsed();

    let (mut passed, mut skipped, mut failed) = (0, 0, 0);
    for test in &tests {
      match test.outcome {
        run::OutcomeKind::Pass { .. } => passed += 1,
        run::OutcomeKind::Skip { .. } => skipped += 1,
        _ => failed += 1,
      };
    }

    TestSummary {
      duration,
      passed,
      skipped,
      failed,
      tests,
    }
  }
}
