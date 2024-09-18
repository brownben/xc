//! Print outcomes to the terminal

use crate::{
  discovery::DiscoveredTests,
  json,
  run::{OutcomeKind, TestOutcome},
  TestSummary,
};

use anstream::eprintln;
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::{OwoColorize, Style};
use std::{fmt, io, time::Duration};

pub fn heading(python_version: &str) {
  eprint!("{}", "xc 🏃".bold().blue());
  eprintln!("{}", format!(" (Python {python_version})").dimmed());
}

pub fn discovery(tests: &DiscoveredTests) {
  eprintln!(
    "   Found {} tests from {} files in {:.2}s",
    tests.tests.len().bold(),
    tests.file_count.bold(),
    tests.duration.as_secs_f64()
  );
}

pub fn test_result(test: &TestOutcome) -> io::Result<()> {
  // Buffer the output so it is all written at once, and not broken by different threads
  use io::Write;
  let mut w = io::BufWriter::new(io::stderr());

  match &test.outcome {
    OutcomeKind::Skip { .. } => write!(w, "{:>10} ", "SKIP".bold().yellow())?,
    OutcomeKind::Pass { .. } => write!(w, "{:>10} ", "PASS".bold().green())?,
    OutcomeKind::Fail { .. } => write!(w, "{:>10} ", "FAIL".bold().red())?,
    _ => write!(w, "{:>10} ", "ERROR".bold().red())?,
  }

  if let Some(time) = test.time() {
    write!(w, "[{:>8.3?}s] ", time.as_secs_f64())?;
  } else {
    write!(w, "[   0.000s] ")?;
  }

  write!(w, "{} ", test.file().display().cyan())?;

  if let Some(ref suite) = test.suite() {
    write!(w, "{}.", suite.blue())?;
  }
  write!(w, "{}", test.name().bold().blue())?;

  writeln!(w)?;

  w.flush()
}

pub fn results_summary(results: &TestSummary) {
  summary(results);
  for result in &results.tests {
    if result.is_fail() {
      error(result);
    }
  }
}

pub fn json_results(results: &TestSummary) {
  let stdout = io::stdout().lock();
  let output: Vec<_> = results.tests.iter().map(json::TestOutput::from).collect();

  serde_json::to_writer(stdout, &output).unwrap();
}

fn summary(summary: &TestSummary) {
  let summary_style = match () {
    () if summary.run() == 0 => Style::new().bold().yellow(),
    () if summary.failed == 0 => Style::new().bold().green(),
    () => Style::new().bold().red(),
  };

  eprintln!("------------");
  eprint!("{:>10} ", "Summary".style(summary_style));
  eprint!("[{:>8.3?}s] ", summary.duration.as_secs_f64());

  eprint!("{} tests run: ", summary.run().bold());
  eprint!("{} ", summary.passed.bold());
  eprint!("{}", "passed".bold().green());

  if summary.failed != 0 {
    eprint!(", {} ", summary.failed.bold());
    eprint!("{}", "failed".bold().red());
  }

  eprint!(", {} ", summary.skipped.bold());
  eprint!("{}", "skipped".bold().green());

  eprintln!();
}

fn error(test: &TestOutcome) {
  debug_assert!(test.is_fail());

  eprint!("\n{}", "FAIL: ".bold().red());
  if let Some(ref suite) = test.suite() {
    eprint!("{}.", suite.red());
  }
  eprint!("{}", test.name().red());
  eprintln!(
    " {}{}{}",
    "(".dimmed(),
    test.file().display().cyan(),
    ")".dimmed()
  );

  if let OutcomeKind::ExpectedFailure { .. } = test.outcome {
    let message = "Expected test to fail, but it passed";
    eprintln!("{}: {message}\n", "ExpectedFailure".bold());
    return;
  }
  if let OutcomeKind::TestNotFound = test.outcome {
    let message = "Could not find test. This is likely a problem in testy.";
    eprintln!("{}: {message}\n", "TestNotFound".bold());
    return;
  }

  let error = test.error().expect("variants without error handled");
  eprintln!("{}: {}\n", error.kind.bold(), error.message);

  if let Some(traceback) = &error.traceback {
    print_frame(
      "Traceback",
      traceback.frames.iter().map(|frame| {
        format!(
          "{} ({}:{})",
          frame.function,
          frame.file.display().dimmed(),
          frame.line.dimmed(),
        )
      }),
    );
  }

  if !error.stdout.is_empty() {
    print_frame("Stdout", error.stdout.lines());
  }
  if !error.stderr.is_empty() {
    print_frame("Stderr", error.stderr.lines());
  }
}

fn print_frame(title: &str, body: impl Iterator<Item = impl fmt::Display>) {
  eprintln!(
    "{}{}{}",
    "╭─ ".dimmed(),
    title.bold().dimmed(),
    ":".dimmed()
  );
  for line in body {
    eprintln!("{}  {line}", "│".dimmed());
  }
  eprintln!("{}", "╰─".dimmed());
}

pub fn create_progress_bar(length: usize) -> ProgressBar {
  let length = length.try_into().unwrap();

  let template = "{prefix:>10} [{elapsed:>9.3}] {wide_bar} {pos:>6}/{len:6}";
  let progress_bar_style = ProgressStyle::with_template(template).unwrap();

  let progress_bar = ProgressBar::new(length).with_style(progress_bar_style);
  progress_bar.enable_steady_tick(Duration::from_millis(250));

  progress_bar
}
