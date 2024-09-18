use crate::{
  discovery::Test,
  python::{self, Error, PyObject},
};

use std::{
  ffi::CStr,
  ops,
  time::{Duration, Instant},
};

/// The result of a test being run
#[derive(Debug, Clone)]
pub struct TestOutcome<'tests> {
  test: &'tests Test,
  pub outcome: OutcomeKind,
}
impl TestOutcome<'_> {
  pub fn time(&self) -> Option<Duration> {
    match self.outcome {
      OutcomeKind::Pass { time }
      | OutcomeKind::Fail { time, .. }
      | OutcomeKind::Error { time, .. }
      | OutcomeKind::ExpectedFailure { time } => Some(time),
      _ => None,
    }
  }

  pub fn is_fail(&self) -> bool {
    !matches!(
      self.outcome,
      OutcomeKind::Pass { .. } | OutcomeKind::Skip { .. }
    )
  }

  pub fn error(&self) -> Option<&Error> {
    match &self.outcome {
      OutcomeKind::Fail { error, .. }
      | OutcomeKind::Error { error, .. }
      | OutcomeKind::ModuleError { error } => Some(error),
      _ => None,
    }
  }
}
impl ops::Deref for TestOutcome<'_> {
  type Target = Test;

  fn deref(&self) -> &Self::Target {
    self.test
  }
}

/// The different outcomes of running a test
#[allow(unused)]
#[derive(Debug, Clone)]
pub enum OutcomeKind {
  /// Test ran successfully with no errors
  Pass { time: Duration },
  /// The test was skipped, and not run
  Skip { reason: String },
  /// An assertion error was raised
  Fail { error: Error, time: Duration },
  /// Any other exception was raised
  Error { error: Error, time: Duration },
  /// Problem setting up module before the test was run
  ModuleError { error: Error },
  /// Expected the test to fail but it succeeded
  ExpectedFailure { time: Duration },
  /// Couldn't find test (likely due to static test def being changed at runtime)
  TestNotFound,
}

/// Executes the test as described by the [`TestToRun`]
pub fn test(test: &Test) -> TestOutcome {
  TestOutcome {
    test,
    outcome: match test {
      Test::Function { .. } => test_function(test),
      Test::Method { .. } => test_method(test),
    },
  }
}

fn test_method(test: &Test) -> OutcomeKind {
  let start_time = Instant::now();
  let module = match python::execute_file(test.file()) {
    Ok(module) => module,
    Err(error) => return OutcomeKind::ModuleError { error },
  };

  let Ok(class) = module.get_attr(test.suite().unwrap()) else {
    return OutcomeKind::TestNotFound;
  };
  let class_instance = match class.call() {
    Ok(class_instance) => class_instance,
    Err(error) => return OutcomeKind::ModuleError { error },
  };

  if let Some(reason) = has_skip_annotation(&class_instance) {
    return OutcomeKind::Skip { reason };
  };
  let Ok(method) = class_instance.get_attr(test.name()) else {
    return OutcomeKind::TestNotFound;
  };
  if let Some(reason) = has_skip_annotation(&method) {
    return OutcomeKind::Skip { reason };
  }

  let expecting_failure = is_expecting_failure(&method);
  if let Err(error) = call_optional_method(&class_instance, c"setUp") {
    return OutcomeKind::ModuleError { error };
  };
  let test_result = method.call();
  if let Err(error) = call_optional_method(&class_instance, c"tearDown") {
    return OutcomeKind::ModuleError { error };
  };
  let time = start_time.elapsed();

  match test_result {
    Ok(_) if expecting_failure => OutcomeKind::ExpectedFailure { time },
    Err(_) if expecting_failure => OutcomeKind::Pass { time },
    Ok(_) => OutcomeKind::Pass { time },
    Err(error) if !error.is_assertion_error() => OutcomeKind::Error { time, error },
    Err(error) => OutcomeKind::Fail { error, time },
  }
}

fn test_function(test: &Test) -> OutcomeKind {
  let start_time = Instant::now();
  let module = match python::execute_file(test.file()) {
    Ok(module) => module,
    Err(error) => return OutcomeKind::ModuleError { error },
  };

  let Ok(function) = module.get_attr(test.name()) else {
    return OutcomeKind::TestNotFound;
  };

  if let Some(reason) = has_skip_annotation(&function) {
    return OutcomeKind::Skip { reason };
  }

  let expecting_failure = is_expecting_failure(&function);
  let test_result = function.call();
  let time = start_time.elapsed();

  match test_result {
    Ok(_) if expecting_failure => OutcomeKind::ExpectedFailure { time },
    Err(_) if expecting_failure => OutcomeKind::Pass { time },
    Ok(_) => OutcomeKind::Pass { time },
    Err(error) if !error.is_assertion_error() => OutcomeKind::Error { time, error },
    Err(error) => OutcomeKind::Fail { error, time },
  }
}

/// Checks a [`PyObject`] for the annotation to skip the test, and returns the set reason for skipping as a string
fn has_skip_annotation(object: &PyObject) -> Option<String> {
  const SKIP_ATTRIBUTE: &CStr = c"__unittest_skip__";
  const SKIP_REASON_ATTRIBUTE: &CStr = c"__unittest_skip_why__";

  if object.has_truthy_attr(SKIP_ATTRIBUTE) {
    let reason = object
      .get_attr_cstr(SKIP_REASON_ATTRIBUTE)
      .map(|x| x.to_string())
      .unwrap_or_default();

    Some(reason)
  } else {
    None
  }
}

/// Checks a [`PyObject`] for the annotation for expecting a failure
fn is_expecting_failure(object: &PyObject) -> bool {
  const EXPECT_ERROR_ATTRIBUTE: &CStr = c"__unittest_expecting_failure__";
  object.has_truthy_attr(EXPECT_ERROR_ATTRIBUTE)
}

fn call_optional_method(object: &PyObject, method: &CStr) -> Result<(), Error> {
  if object.has_attr(method) {
    let method = object.get_attr_cstr(method)?;
    let _call_result = method.call()?;
  }

  Ok(())
}
