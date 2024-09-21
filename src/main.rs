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
  process::{self, ExitCode},
  time::{Duration, Instant},
};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
  /// List of files or directories to test
  #[clap(default_value = ".")]
  pub paths: Vec<std::path::PathBuf>,

  /// List of files or directories to exclude from testing
  #[clap(long, value_name = "FILE_PATTERN")]
  pub exclude: Vec<std::path::PathBuf>,

  #[clap(flatten)]
  pub coverage: CoverageArgs,

  /// Don't stop executing tests after one has failed
  #[clap(long, default_value_t = false)]
  pub no_fail_fast: bool,

  /// Output results as JSON to stdout
  #[clap(long, default_value_t = false)]
  pub json: bool,
}

#[derive(clap::Args, Debug)]
struct CoverageArgs {
  /// Enable line coverage gathering and reporting
  #[clap(long = "coverage", default_value_t = false)]
  pub enabled: bool,

  /// List of paths, used to determine files to report coverage for
  #[clap(
    name = "coverage-include",
    long = "coverage-include",
    value_name = "FILE_PATTERN",
    help_heading = "Coverage"
  )]
  pub include: Vec<std::path::PathBuf>,

  /// List of paths, used to omit files and/or directories from coverage reporting
  #[clap(
    name = "coverage-exclude",
    long = "coverage-exclude",
    value_name = "FILE_PATTERN",
    help_heading = "Coverage"
  )]
  pub exclude: Vec<std::path::PathBuf>,
}

fn main() -> ExitCode {
  let args = Args::parse();
  print::heading(&python::version());

  // Discover tests
  let discovered = discovery::find_tests(&args.paths, &args.exclude);
  print::discovery(&discovered);

  // Main Python interpreter must be initialized in the main thread
  let _interpreter = python::Interpreter::initialize();

  // Run tests
  let progress_bar = print::create_progress_bar(discovered.test_count);
  let results: TestSummary = discovered
    .tests
    .par_iter()
    .map(|test| {
      let mut subinterpreter = python::SubInterpreter::new();

      if args.coverage.enabled {
        subinterpreter.enable_coverage();
      }

      let outcome = subinterpreter.run(|| run::test(test));
      let coverage = subinterpreter.get_coverage();

      (outcome, coverage)
    })
    .inspect(|(outcome, _coverage)| {
      progress_bar.suspend(|| print::test_result(outcome).unwrap());
      progress_bar.inc(1);

      if !args.no_fail_fast && outcome.is_fail() {
        progress_bar.finish_and_clear();
        print::error(outcome).unwrap();
        process::exit(1);
      }
    })
    .collect();
  progress_bar.finish_and_clear();

  // Report results
  print::results_summary(&results);
  if args.json {
    print::json_results(&results);
  };

  let successful = results.failed == 0 && results.passed > 0;

  if args.coverage.enabled && successful {
    let coverage_include = if args.coverage.include.is_empty() {
      &args.paths
    } else {
      &args.coverage.include
    };
    let coverage_exclude = if args.coverage.exclude.is_empty() {
      &args.exclude
    } else {
      &args.coverage.exclude
    };

    let possible_lines = coverage::get_executable_lines(coverage_include, coverage_exclude);
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
