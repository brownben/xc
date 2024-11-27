use crate::{
  discovery::Test,
  python::{
    objects::{PyDict, PyError, PyObject, PyTuple},
    ActiveInterpreter,
  },
};

use serde::{Deserialize, Serialize};
use std::{
  ops, path,
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
impl OutcomeKind {
  pub fn module_error(error: PyError) -> Self {
    Self::ModuleError {
      error: error.into(),
    }
  }
}

/// An error which occurred whilst executing Python code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Error {
  pub kind: String,
  pub message: String,

  pub traceback: Option<Traceback>,

  pub stdout: Option<String>,
  pub stderr: Option<String>,
}
impl Error {
  pub fn is_assertion_error(&self) -> bool {
    self.kind == "AssertionError"
  }
  pub fn is_skip_exception(&self) -> bool {
    self.kind.starts_with("Skip")
  }
}
impl From<PyError> for Error {
  fn from(error: PyError) -> Self {
    // SAFETY: assume that the GIl is held
    let interpreter = unsafe { ActiveInterpreter::new() };
    let (stdout, stderr) = interpreter.get_captured_output();

    Self {
      kind: error.type_name(),
      message: error.to_string(),

      traceback: Traceback::from(&error),
      stdout,
      stderr,
    }
  }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Traceback {
  pub frames: Vec<TracebackFrame>,
}
impl Traceback {
  fn from(error: &PyError) -> Option<Self> {
    let mut traceback = Self { frames: Vec::new() };
    traceback.add_frame(&error.get_traceback()?).ok()?;
    Some(traceback)
  }

  fn add_frame(&mut self, frame: &PyObject) -> Result<(), PyError> {
    let code_object = frame.get_attr_cstr(c"tb_frame")?.get_attr_cstr(c"f_code")?;

    let line = frame.get_attr_cstr(c"tb_lineno")?.as_long();
    let function = code_object.get_attr_cstr(c"co_name")?.to_string();
    let file = code_object.get_attr_cstr(c"co_filename")?.to_string();

    self.frames.push(TracebackFrame {
      line,
      function,
      file: path::PathBuf::from(file),
    });

    if let Ok(frame) = frame.get_attr_cstr(c"tb_next") {
      _ = self.add_frame(&frame);
    }

    Ok(())
  }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[must_use]
pub struct TracebackFrame {
  pub line: i32,
  pub function: String,
  pub file: path::PathBuf,
}

/// Executes the test as described by the [`Test`]
pub fn test<'test>(python: &ActiveInterpreter, test: &'test Test) -> TestOutcome<'test> {
  TestOutcome {
    test,
    outcome: match test {
      Test::Function { .. } => test_function(python, test),
      Test::Method { .. } => test_method(python, test),
    },
  }
}

fn test_method(python: &ActiveInterpreter, test: &Test) -> OutcomeKind {
  let start_time = Instant::now();
  let module = match python.execute_file(test.file()) {
    Ok(module) => module,
    Err(error) => return OutcomeKind::module_error(error),
  };

  let suite_name = python.new_string(test.suite().unwrap());
  let Ok(class) = module.get_attr(&suite_name) else {
    return OutcomeKind::TestNotFound;
  };
  let class_instance = match class.call() {
    Ok(class_instance) => class_instance,
    Err(error) => return OutcomeKind::module_error(error),
  };

  if let Some(reason) = has_skip_annotation(python, &class_instance) {
    return OutcomeKind::Skip { reason };
  };
  let test_name = python.new_string(test.name());
  let Ok(method) = class_instance.get_attr(&test_name) else {
    return OutcomeKind::TestNotFound;
  };
  if let Some(reason) = has_skip_annotation(python, &method) {
    return OutcomeKind::Skip { reason };
  }

  let expecting_failure = is_expecting_failure(python, &method);
  if let Err(error) = call_optional_method(python, &class_instance, "setUp") {
    return OutcomeKind::module_error(error);
  };
  let test_result = method.call();
  if let Err(error) = call_optional_method(python, &class_instance, "tearDown") {
    return OutcomeKind::module_error(error);
  };
  let time = start_time.elapsed();

  match test_result {
    Ok(_) if expecting_failure => OutcomeKind::ExpectedFailure { time },
    Err(_) if expecting_failure => OutcomeKind::Pass { time },
    Ok(_) => OutcomeKind::Pass { time },
    Err(error) => {
      let error = Error::from(error);

      if error.is_skip_exception() {
        let reason = error.message;
        OutcomeKind::Skip { reason }
      } else if error.is_assertion_error() {
        OutcomeKind::Fail { error, time }
      } else {
        OutcomeKind::Error { error, time }
      }
    }
  }
}

fn test_function(python: &ActiveInterpreter, test: &Test) -> OutcomeKind {
  let start_time = Instant::now();
  let module = match python.execute_file(test.file()) {
    Ok(module) => module,
    Err(error) => return OutcomeKind::module_error(error),
  };

  let test_name = python.new_string(test.name());
  let Ok(function) = module.get_attr(&test_name) else {
    return OutcomeKind::TestNotFound;
  };

  if let Some(reason) = has_skip_annotation(python, &function) {
    return OutcomeKind::Skip { reason };
  }

  let expecting_failure = is_expecting_failure(python, &function);
  let test_result = function.call();
  let time = start_time.elapsed();

  match test_result {
    Ok(_) if expecting_failure => OutcomeKind::ExpectedFailure { time },
    Err(_) if expecting_failure => OutcomeKind::Pass { time },
    Ok(_) => OutcomeKind::Pass { time },
    Err(error) => {
      let error = Error::from(error);

      if error.is_skip_exception() {
        let reason = error.message;
        OutcomeKind::Skip { reason }
      } else if error.is_assertion_error() {
        OutcomeKind::Fail { error, time }
      } else {
        OutcomeKind::Error { error, time }
      }
    }
  }
}

/// Checks a [`PyObject`] for the annotation to skip the test, and returns the set reason for skipping as a string
fn has_skip_annotation(python: &ActiveInterpreter, object: &PyObject) -> Option<String> {
  if has_truthy_attr(python, object, "__unittest_skip__") {
    let reason = object
      .get_attr_cstr(c"__unittest_skip_why__")
      .map(|x| x.to_string())
      .unwrap_or_default();

    return Some(reason);
  }

  if let Ok(pytest_marks) = object.get_attr_cstr(c"pytestmark") {
    for mark in pytest_marks.into_iter() {
      let mark_name = mark.get_attr_cstr(c"name").ok()?.to_string();

      let should_skip = match mark_name.as_str() {
        "skip" => true,
        "skipIf" => unsafe {
          PyTuple::from_object_unchecked(mark.get_attr_cstr(c"args").ok()?)
            .get_item_unchecked(0)
            .is_truthy()
        },
        _ => false,
      };

      if should_skip {
        let reason = if let Ok(kwargs) = mark.get_attr_cstr(c"kwargs") {
          unsafe {
            PyDict::from_object_unchecked(kwargs)
              .get_item(&python.new_string("reason"))
              .map(|item| item.to_string())
              .unwrap_or_default()
          }
        } else {
          String::new()
        };

        return Some(reason);
      }
    }
  }

  None
}

/// Checks a [`PyObject`] for the annotation for expecting a failure
fn is_expecting_failure(python: &ActiveInterpreter, object: &PyObject) -> bool {
  if has_truthy_attr(python, object, "__unittest_expecting_failure__") {
    return true;
  }

  if let Ok(pytest_marks) = object.get_attr_cstr(c"pytestmark") {
    for mark in pytest_marks.into_iter() {
      let mark_name = mark.get_attr_cstr(c"name").unwrap().to_string();

      if mark_name == "xfail" {
        return unsafe {
          PyTuple::from_object_unchecked(mark.get_attr_cstr(c"args").unwrap())
            .get_item_unchecked(0)
            .is_truthy()
        };
      }
    }
  }

  false
}

fn call_optional_method(
  python: &ActiveInterpreter,
  object: &PyObject,
  method: &str,
) -> Result<(), PyError> {
  let method_name = python.new_string(method);

  if object.has_attr(&method_name) {
    let method = unsafe { object.get_attr_unchecked(&method_name) };
    let _call_result = method.call()?;
  }

  Ok(())
}

fn has_truthy_attr(python: &ActiveInterpreter, object: &PyObject, attribute: &str) -> bool {
  let attribute = python.new_string(attribute);

  let has_attr = object.has_attr(&attribute);

  if !has_attr {
    return false;
  }

  object.get_attr(&attribute).unwrap().is_truthy()
}
