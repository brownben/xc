#![warn(clippy::pedantic)]
#![allow(clippy::unsafe_derive_deserialize)]

mod config;
mod coverage;
mod discovery;
mod output;
mod python;
mod run;

use python::Interpreter;
use rayon::prelude::*;
use run::TestOutcome;
use std::{
  process::{self, ExitCode},
  time::{Duration, Instant},
};

fn main() -> ExitCode {
  let settings = config::read_settings();

  let mut reporter = output::new_reporter(settings.output);
  reporter.initialize(python::version());

  // Discover tests
  let discovered = discovery::find_tests(&settings.paths, &settings.exclude);
  reporter.discovered(&discovered);

  // Main Python interpreter must be initialized in the main thread
  let mut interpreter = python::MainInterpreter::initialize();
  interpreter.with_gil(|python| {
    // The decimal module crashes Python 3.12 if it is initialised multiple times
    // If not initialised in the base interpreter, if a subinterpreter imports it it will crash
    _ = python.import_module(c"decimal");
  });

  // Run tests
  let results: TestSummary = discovered
    .tests
    .par_iter()
    .map(|test| {
      let mut subinterpreter = python::SubInterpreter::new(&interpreter);

      if settings.coverage.enabled {
        subinterpreter.enable_coverage();
      }

      let outcome = subinterpreter.with_gil(|python| {
        python.capture_output();
        python.add_parent_module_to_path(test.file());

        run::test(python, test)
      });
      let coverage = subinterpreter.get_coverage();

      (outcome, coverage)
    })
    .inspect(|(outcome, _coverage)| {
      reporter.result(outcome);

      if !settings.no_fail_fast && outcome.is_fail() {
        reporter.fail_fast_error(outcome);
        process::exit(1);
      }
    })
    .collect();

  // Report results
  reporter.summary(&results);

  let successful = results.failed == 0 && results.passed > 0;

  if settings.coverage.enabled && successful {
    let coverage_include = if settings.coverage.include.is_empty() {
      &settings.paths
    } else {
      &settings.coverage.include
    };
    let coverage_exclude = if settings.coverage.exclude.is_empty() {
      &settings.exclude
    } else {
      &settings.coverage.exclude
    };

    let possible_lines =
      coverage::get_executable_lines(&interpreter, coverage_include, coverage_exclude);
    coverage::print_summary(&possible_lines, &results.executed_lines);
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
