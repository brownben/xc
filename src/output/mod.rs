use crate::{discovery::DiscoveredTests, run::TestOutcome, OutputFormat, TestSummary};

pub trait Reporter: Sync {
  fn initialize(&mut self, _python_version: String) {}
  fn discovered(&mut self, _discovered: &DiscoveredTests) {}
  fn result(&self, _result: &TestOutcome) {}
  fn fail_fast_error(&self, _result: &TestOutcome) {}
  fn summary(&mut self, _summary: &TestSummary) {}
}

pub(crate) mod json;
use json::JSONReporter;

mod standard;
use standard::ProgressReporter;

pub fn new_reporter(format: OutputFormat) -> Box<dyn Reporter> {
  match format {
    OutputFormat::Standard => Box::new(ProgressReporter::new()),
    OutputFormat::Json => Box::new(JSONReporter),
  }
}
