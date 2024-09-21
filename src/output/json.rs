use super::Reporter;
use crate::{
  python,
  run::{OutcomeKind, TestOutcome},
};

use serde::{Deserialize, Serialize};
use std::{
  io::{self, Write},
  path::PathBuf,
  time::Duration,
};

pub struct JSONReporter;
impl Reporter for JSONReporter {
  fn result(&self, result: &TestOutcome) {
    let mut stdout = io::BufWriter::new(io::stdout());
    let result = JSONTestOutput::from(result);

    serde_json::to_writer(&mut stdout, &result).unwrap();
    writeln!(&mut stdout).unwrap();
    stdout.flush().unwrap();
  }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JSONTestOutput {
  pub file: PathBuf,
  pub test_identifier: String,
  pub outcome: Outcome,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub error: Option<python::Error>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub time: Option<Duration>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
  Pass,
  Fail,
  Skip,
  ExpectedFailure,
  NonTestFail,
}

impl From<&TestOutcome<'_>> for JSONTestOutput {
  fn from(test: &TestOutcome) -> Self {
    Self {
      file: test.file().to_owned(),
      test_identifier: test.identifier(),
      outcome: (&test.outcome).into(),
      error: test.error().cloned(),
      time: test.time(),
    }
  }
}
impl From<&OutcomeKind> for Outcome {
  fn from(kind: &OutcomeKind) -> Self {
    match kind {
      OutcomeKind::Pass { .. } => Self::Pass,
      OutcomeKind::Skip { .. } => Self::Skip,
      OutcomeKind::Fail { .. } | OutcomeKind::Error { .. } => Self::Fail,
      OutcomeKind::ExpectedFailure { .. } => Self::ExpectedFailure,
      OutcomeKind::ModuleError { .. } | OutcomeKind::TestNotFound => Self::NonTestFail,
    }
  }
}
