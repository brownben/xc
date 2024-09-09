//! Print outcomes to the terminal

use crate::discovery::DiscoveredTests;

use anstream::eprintln;
use owo_colors::OwoColorize;

pub fn heading() {
  eprintln!("{}", "xc 🏃".bold().blue());
}

pub fn discovery(tests: &DiscoveredTests) {
  eprintln!(
    "   Found {} tests from {} files in {:.2}s",
    tests.tests.len().bold(),
    tests.file_count.bold(),
    tests.duration.as_secs_f64()
  );
}
