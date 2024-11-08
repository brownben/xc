use crate::output::json::{JSONTestOutput, Outcome};
use assert_cmd::Command;

macro_rules! run_tests {
  ($file:expr) => {{
    let cmd_output = Command::cargo_bin(env!("CARGO_PKG_NAME"))
      .unwrap()
      .arg($file)
      .arg("--output=json")
      .arg("--no-fail-fast")
      .output()
      .unwrap();

    let stdout = String::from_utf8(cmd_output.stdout).unwrap();

    stdout
      .lines()
      .map(|line| serde_json::from_str::<JSONTestOutput>(&line).unwrap())
      .collect::<Vec<JSONTestOutput>>()
  }};
}

#[test]
fn simple_function() {
  let results = run_tests!("./examples/simple_function.py");

  assert_eq!(results.len(), 1);
  assert!(results.iter().all(|x| x.outcome == Outcome::Pass));
}

#[test]
fn simple_method() {
  let results = run_tests!("./examples/simple_method.py");

  assert_eq!(results.len(), 3);
  assert!(results.iter().all(|x| x.outcome == Outcome::Pass));
}

#[test]
fn test_times() {
  let start_time = std::time::Instant::now();
  let results = run_tests!("./examples/test_times.py");
  let execution_time = start_time.elapsed();

  assert_eq!(results.len(), 5);
  assert!(results.iter().all(|x| x.outcome == Outcome::Pass));

  // Check that the tests are run in parallel
  let total_time = results.iter().flat_map(|result| result.time).sum();
  assert!(execution_time < total_time);
}

#[test]
fn nested_package_import() {
  let results = run_tests!("./examples/package/test_times.py");

  assert_eq!(results.len(), 1);
  assert!(results.iter().all(|x| x.outcome == Outcome::Pass));
}

#[ignore = "takes too long"]
#[test]
fn long_running() {
  let start_time = std::time::Instant::now();
  let results = run_tests!("./examples/long_running.py");
  let execution_time = start_time.elapsed();

  assert_eq!(results.len(), 3);
  assert!(results.iter().all(|x| x.outcome == Outcome::Pass));

  // Check that the tests are run in parallel
  let total_time = results.iter().flat_map(|result| result.time).sum();
  assert!(execution_time < total_time);
}

#[test]
fn skip_test() {
  let results = run_tests!("./examples/skip_test.py");

  assert_eq!(results.len(), 4);
  assert!(results.iter().all(|x| x.outcome == Outcome::Skip));
}

#[test]
fn expected_error() {
  let results = run_tests!("./examples/expected_error.py");

  assert_eq!(results.len(), 2);
  assert_eq!(results.len(), 2);
  let (test_will_fail, test_wont_fail) = if results[0].test_identifier == "test_will_fail" {
    (&results[0], &results[1])
  } else {
    (&results[1], &results[0])
  };

  assert_eq!(test_will_fail.outcome, Outcome::Pass);
  assert_eq!(test_wont_fail.outcome, Outcome::ExpectedFailure);
}

#[test]
fn failing_test() {
  let results = run_tests!("./examples/failing_test.py");

  assert_eq!(results.len(), 2);
  assert!(results.iter().all(|x| x.outcome == Outcome::Fail));

  let (regular_fail, other_error) = if results[0].test_identifier == "test_regular_fail" {
    (&results[0], &results[1])
  } else {
    (&results[1], &results[0])
  };

  let error = regular_fail.error.as_ref().unwrap();
  assert_eq!(error.kind, "AssertionError");

  let error = other_error.error.as_ref().unwrap();
  assert_eq!(error.kind, "TypeError");
}

#[test]
fn invalid_method() {
  let results = run_tests!("./examples/invalid_method.py");

  assert_eq!(results.len(), 1);
  assert!(results.iter().all(|x| x.outcome == Outcome::Fail));

  let error = results.first().unwrap().error.as_ref().unwrap();
  assert_eq!(error.kind, "TypeError");
  assert_eq!(
    error.message,
    "TestAdd.test_add() takes 0 positional arguments but 1 was given"
  );
}

#[test]
fn captures_stdout() {
  let results = run_tests!("./examples/captures_stdout.py");

  assert_eq!(results.len(), 2);
  let (test_stdout, test_stderr) = if results[0].test_identifier == "test_stdout" {
    (&results[0], &results[1])
  } else {
    (&results[1], &results[0])
  };

  assert_eq!(test_stdout.outcome, Outcome::Fail);
  assert_eq!(
    test_stdout.error.as_ref().unwrap().stdout,
    "hello world into stdout\n"
  );
  assert_eq!(test_stdout.error.as_ref().unwrap().stderr, "");

  assert_eq!(test_stderr.outcome, Outcome::Fail);
  assert_eq!(test_stderr.error.as_ref().unwrap().stdout, "");
  assert_eq!(
    test_stderr.error.as_ref().unwrap().stderr,
    "hello world\ninto stderr"
  );
}

#[test]
fn import_decimal_module() {
  let results = run_tests!("./examples/import_decimal.py");

  assert_eq!(results.len(), 2);
  assert!(results.iter().all(|x| x.outcome == Outcome::Pass));
}
