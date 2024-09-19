#![warn(clippy::pedantic)]
#![allow(clippy::unsafe_derive_deserialize)]
#![deny(unsafe_code)]

mod coverage;
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

  /// Calculate coverage information for the tests
  #[clap(long, default_value_t = false)]
  pub coverage: bool,

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
    .map(|test| {
      let mut subinterpreter = python::SubInterpreter::new();

      if args.coverage {
        subinterpreter.enable_coverage();
      }

      let outcome = subinterpreter.run(|| run::test(test));
      let coverage = subinterpreter.get_coverage();

      (outcome, coverage)
    })
    .map(|(outcome, coverage)| {
      if args.no_fail_fast {
        return Some((outcome, coverage));
      }

      if seen_failed_test.load(Relaxed) {
        return None;
      }

      if outcome.is_fail() {
        seen_failed_test.store(true, Relaxed);
      }

      Some((outcome, coverage))
    })
    .while_some()
    .inspect(|(result, _coverage)| {
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

  let successful = results.failed == 0 && results.passed > 0;

  if args.coverage && successful {
    let possible_lines = coverage::get_executable_lines(&args.paths);
    print::coverage_summary(&possible_lines, &results.executed_lines);
  }

  if successful {
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
  pub executed_lines: coverage::Lines,
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
impl<'tests> FromParallelIterator<(TestOutcome<'tests>, Option<coverage::Lines>)>
  for TestSummary<'tests>
{
  fn from_par_iter<
    T: IntoParallelIterator<Item = (TestOutcome<'tests>, Option<coverage::Lines>)>,
  >(
    iter: T,
  ) -> Self {
    let start_time = Instant::now();
    let (tests, executed_lines): (Vec<_>, coverage::Lines) = iter.into_par_iter().unzip();
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
      executed_lines,
    }
  }
}
