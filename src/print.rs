//! Print outcomes to the terminal

use crate::{
  coverage,
  discovery::DiscoveredTests,
  json,
  run::{OutcomeKind, TestOutcome},
  TestSummary,
};

use anstream::eprintln;
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::{OwoColorize, Style};
use std::{collections::BTreeSet, fmt, io, time::Duration};

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
      error(result).unwrap();
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
  eprint!("{}", "skipped".bold().yellow());

  eprintln!();
}

pub fn error(test: &TestOutcome) -> io::Result<()> {
  debug_assert!(test.is_fail());

  use io::Write;
  let mut w = io::BufWriter::new(io::stderr());

  write!(w, "\n{}", "FAIL: ".bold().red())?;
  if let Some(ref suite) = test.suite() {
    write!(w, "{}.", suite.red())?;
  }
  write!(w, "{}", test.name().red())?;
  writeln!(
    w,
    " {}{}{}",
    "(".dimmed(),
    test.file().display().cyan(),
    ")".dimmed()
  )?;

  if let OutcomeKind::ExpectedFailure { .. } = test.outcome {
    let message = "Expected test to fail, but it passed";
    return writeln!(w, "{}: {message}\n", "ExpectedFailure".bold());
  }
  if let OutcomeKind::TestNotFound = test.outcome {
    let message = "Could not find test. This is likely a problem in testy.";
    return writeln!(w, "{}: {message}\n", "TestNotFound".bold());
  }

  let error = test.error().expect("variants without error handled");
  writeln!(w, "{}: {}\n", error.kind.bold(), error.message)?;

  if let Some(traceback) = &error.traceback {
    print_frame(
      &mut w,
      "Traceback",
      traceback.frames.iter().map(|frame| {
        format!(
          "{} ({}:{})",
          frame.function,
          frame.file.display().dimmed(),
          frame.line.dimmed(),
        )
      }),
    )?;
  }

  if !error.stdout.is_empty() {
    print_frame(&mut w, "Stdout", error.stdout.lines())?;
  }
  if !error.stderr.is_empty() {
    print_frame(&mut w, "Stderr", error.stderr.lines())?;
  }

  w.flush()
}

fn print_frame(
  w: &mut dyn io::Write,
  title: &str,
  body: impl Iterator<Item = impl fmt::Display>,
) -> io::Result<()> {
  writeln!(
    w,
    "{}{}{}",
    "╭─ ".dimmed(),
    title.bold().dimmed(),
    ":".dimmed()
  )?;
  for line in body {
    writeln!(w, "{}  {line}", "│".dimmed())?;
  }
  writeln!(w, "{}", "╰─".dimmed())
}

pub fn create_progress_bar(length: usize) -> ProgressBar {
  let length = length.try_into().unwrap();

  let template = "{prefix:>10} [{elapsed:>9.3}] {wide_bar} {pos:>6}/{len:6}";
  let progress_bar_style = ProgressStyle::with_template(template).unwrap();

  let progress_bar = ProgressBar::new(length).with_style(progress_bar_style);
  progress_bar.enable_steady_tick(Duration::from_millis(250));

  progress_bar
}

pub fn coverage_summary(possible: &coverage::Lines, executed: &coverage::Lines) {
  let empty = BTreeSet::new();

  eprintln!("\n{}{}", "╭─ ".dimmed(), "Coverage".bold());
  eprintln!(
    "{}{:55} {}",
    "│  ".dimmed(),
    "File".dimmed().italic(),
    "Lines    Missed  Coverage".dimmed().italic(),
  );

  for (file_name, possible_lines) in possible.iter() {
    let executed_lines = executed.get_lines(file_name).unwrap_or(&empty);
    let covered_lines = possible_lines.intersection(executed_lines).count();
    let total_lines = possible_lines.len();
    let missed_lines = total_lines - covered_lines;

    #[expect(clippy::cast_precision_loss, reason = "line numbers < f64::MAX")]
    let coverage = (covered_lines as f64 / total_lines as f64) * 100.0;

    eprintln!(
      "{}{:55}{:6}{:>10}{:>9.1}%",
      "├─ ".dimmed(),
      file_name,
      total_lines,
      missed_lines,
      coverage,
    );
  }
  eprintln!("{}", "╰──".dimmed());
}
