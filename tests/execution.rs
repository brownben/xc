use assert_cmd::Command;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf, time::Duration};

macro_rules! execution_test {
  ($name:ident) => {
    execution_test!($name, stringify!($name));
  };
  ($name:ident, $path:expr) => {
    #[test]
    fn $name() {
      let test_file_path = concat!("./tests/execution/", $path, ".py");
      let test_file = include_str!(concat!("./execution/", $path, ".py"));

      let expected_results = expected_results(test_file);
      let test_results = run_test(test_file_path);

      for (test_name, (outcome, expected_error)) in &expected_results {
        let Some(result) = test_results
          .iter()
          .find(|result| &result.test_identifier == test_name)
        else {
          let cmd_output = Command::cargo_bin(env!("CARGO_PKG_NAME"))
            .unwrap()
            .arg(test_file_path)
            .arg("--output=json")
            .arg("--no-fail-fast")
            .output()
            .unwrap();
          println!("{cmd_output:?}");
          println!("expected results {expected_results:?} test results {test_results:?}");
          panic!("Expected test '{test_name}' to have been run");
        };

        assert_eq!(
          result.outcome, *outcome,
          "Expected outcome for '{}' to be {:?}, but got {:?}",
          test_name, outcome, result.outcome
        );

        if let Some(expected_error) = expected_error {
          let test_error = &result.error.as_ref().unwrap();

          if let Some(kind) = &expected_error.kind {
            assert_eq!(
              kind, &test_error.kind,
              "Expected error kind for '{}' to be {:?}, but got {:?}",
              test_name, kind, test_error.kind
            );
          }
          if let Some(message) = &expected_error.message {
            assert_eq!(
              message, &test_error.message,
              "Expected error message for '{}' to be {:?}, but got {:?}",
              test_name, message, test_error.message
            );
          }
          if let Some(stdout) = &expected_error.stdout {
            assert_eq!(
              stdout, &test_error.stdout,
              "Expected captured stdout for '{}' to be {:?}, but got {:?}",
              test_name, stdout, test_error.stdout
            );
          };
          if let Some(stderr) = &expected_error.stderr {
            assert_eq!(
              stderr, &test_error.stderr,
              "Expected captured stderr for '{}' to be {:?}, but got {:?}",
              test_name, stderr, test_error.stderr
            );
          };
        }
      }

      // Check that all expected tests were run
      assert_eq!(expected_results.len(), test_results.len());
    }
  };
}

fn expected_results(test_file: &str) -> HashMap<String, (Outcome, Option<ErrorAssertion>)> {
  test_file
    .lines()
    .skip_while(|line| !line.starts_with('-'))
    .take_while(|line| line.starts_with('-'))
    .map(|line| {
      let (test_name, mut outcome) = line.split_once(':').unwrap();
      let test_name = test_name.trim_start_matches("- ").to_string();

      let error = if outcome.contains("FAIL {") {
        let json_text = outcome.trim_start_matches(" FAIL ");
        outcome = "FAIL";
        Some(serde_json::from_str(json_text).unwrap())
      } else {
        None
      };

      (
        test_name,
        (
          match outcome.trim() {
            "PASS" => Outcome::Pass,
            "FAIL" => Outcome::Fail,
            "SKIP" => Outcome::Skip,
            "EXPECTED FAILURE" => Outcome::ExpectedFailure,
            "NON TEST FAIL" => Outcome::NonTestFail,
            _ => panic!("Unknown Outcome: {}", outcome.trim()),
          },
          error,
        ),
      )
    })
    .collect()
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Error {
  pub kind: String,
  pub message: String,
  pub stdout: String,
  pub stderr: String,
  // also has traceback field, but we don't test on that yet
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorAssertion {
  pub kind: Option<String>,
  pub message: Option<String>,
  pub stdout: Option<String>,
  pub stderr: Option<String>,
  // also has traceback field, but we don't test on that yet
}

fn run_test(test_path: &str) -> Vec<TestOutput> {
  let cmd_output = Command::cargo_bin(env!("CARGO_PKG_NAME"))
    .unwrap()
    .arg(test_path)
    .arg("--output=json")
    .arg("--no-fail-fast")
    .output()
    .unwrap();

  let stdout = String::from_utf8(cmd_output.stdout).unwrap();

  stdout
    .lines()
    .map(|line| serde_json::from_str::<TestOutput>(&line).unwrap())
    .collect::<Vec<TestOutput>>()
}

execution_test!(basic_function);
execution_test!(basic_method);
execution_test!(captures_stdout);
execution_test!(expected_error);
execution_test!(failing_test);
execution_test!(imports);
execution_test!(import_submodule, "package/import_submodule");
execution_test!(import_decimal);
execution_test!(invalid_code);
#[cfg(feature = "ci")] // Takes a long time, so don't want it slowing down developement cycles
execution_test!(long_running);
#[cfg(not(feature = "ci"))] // Pytest crashes in CI, with a double free error - don't know why
execution_test!(pytest_marks);
execution_test!(skip_tests);
execution_test!(times); // No tests are in this file, just a standard python file
