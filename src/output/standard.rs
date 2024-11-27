use super::Reporter;
use crate::{
  discovery::DiscoveredTests,
  run::{OutcomeKind, TestOutcome},
  TestSummary,
};

use anstream::eprintln;
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::{OwoColorize, Style};
use std::{
  fmt,
  io::{self, BufWriter, Write},
  time::Duration,
};

pub struct ProgressReporter {
  progress_bar: ProgressBar,
}
impl ProgressReporter {
  pub fn new() -> Self {
    Self {
      progress_bar: ProgressBar::hidden(),
    }
  }
  fn create_progress_bar(&mut self, length: usize) {
    let length = length.try_into().unwrap();

    let template = "{prefix:>10} [{elapsed:>9.3}] {wide_bar} {pos:>6}/{len:6}";
    let progress_bar_style = ProgressStyle::with_template(template).unwrap();

    self.progress_bar = ProgressBar::new(length).with_style(progress_bar_style);
    self
      .progress_bar
      .enable_steady_tick(Duration::from_millis(250));
  }
}
impl Reporter for ProgressReporter {
  fn initialize(&mut self, python_version: String) {
    eprint!("{}", "xc 🏃".bold().blue());
    eprintln!("{}", format!(" (Python {python_version})").dimmed());
  }

  fn discovered(&mut self, discovered: &DiscoveredTests) {
    eprintln!(
      "   Found {} tests from {} files in {:.2}s",
      discovered.tests.len().bold(),
      discovered.file_count.bold(),
      discovered.duration.as_secs_f64()
    );

    self.create_progress_bar(discovered.test_count);
  }

  fn result(&self, result: &TestOutcome) {
    self.progress_bar.suspend(|| {
      // Buffer test output, so multiple threads don't interfere
      let mut w = BufWriter::new(io::stderr());
      test_result(&mut w, result).unwrap();
      w.flush().unwrap();
    });
    self.progress_bar.inc(1);
  }

  fn fail_fast_error(&self, result: &TestOutcome) {
    self.progress_bar.finish_and_clear();

    error(result).unwrap();
  }

  fn summary(&mut self, summary: &TestSummary) {
    self.progress_bar.finish_and_clear();

    summary_heading(summary);
    for result in &summary.tests {
      if result.is_fail() {
        error(result).unwrap();
      }
    }
  }
}

fn test_result(w: &mut dyn io::Write, test: &TestOutcome) -> io::Result<()> {
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

  writeln!(w)
}

fn summary_heading(summary: &TestSummary) {
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

fn error(test: &TestOutcome) -> io::Result<()> {
  debug_assert!(test.is_fail());

  let mut w = BufWriter::new(io::stderr());

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
    let message = "Could not find test. This is likely a problem in xc.";
    return writeln!(w, "{}: {message}\n", "TestNotFound".bold());
  }

  let error = test.error().expect("variants without error handled");
  writeln!(w, "{}: {}\n", error.kind.bold(), error.message)?;

  if let Some(traceback) = &error.traceback {
    frame(
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

  if let Some(stdout) = &error.stdout {
    if !stdout.is_empty() {
      frame(&mut w, "Stdout", stdout.lines())?;
    }
  }
  if let Some(stderr) = &error.stderr {
    if !stderr.is_empty() {
      frame(&mut w, "Stderr", stderr.lines())?;
    }
  }

  w.flush()
}

fn frame(
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
