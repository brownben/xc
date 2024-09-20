//! Convert test results to format for JSON output

use crate::{python::Error, run};

use serde::{Deserialize, Serialize};
use std::{path::PathBuf, time::Duration};

#[derive(Debug, Serialize, Deserialize)]
pub struct TestOutput {
  pub file: PathBuf,
  pub test_identifier: String,
  pub outcome: Outcome,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub error: Option<Error>,
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

impl From<&run::TestOutcome<'_>> for TestOutput {
  fn from(test: &run::TestOutcome) -> Self {
    Self {
      file: test.file().to_owned(),
      test_identifier: test.identifier(),
      outcome: (&test.outcome).into(),
      error: test.error().cloned(),
      time: test.time(),
    }
  }
}
impl From<&run::OutcomeKind> for Outcome {
  fn from(kind: &run::OutcomeKind) -> Self {
    match kind {
      run::OutcomeKind::Pass { .. } => Self::Pass,
      run::OutcomeKind::Skip { .. } => Self::Skip,
      run::OutcomeKind::Fail { .. } | run::OutcomeKind::Error { .. } => Self::Fail,
      run::OutcomeKind::ExpectedFailure { .. } => Self::ExpectedFailure,
      run::OutcomeKind::ModuleError { .. } | run::OutcomeKind::TestNotFound => Self::NonTestFail,
    }
  }
}
