use assert_cmd::Command;
use indoc::indoc;
use regex::Regex;

// as the test order is non-deterministic, we only check the output when there is a single test

#[test]
fn regular_output_passing() {
  let output = Command::cargo_bin(env!("CARGO_PKG_NAME"))
    .unwrap()
    .arg("./tests/output/passing_test.py")
    .output()
    .unwrap();
  let (stdout, stderr) = (clean_output(output.stdout), clean_output(output.stderr));

  assert!(output.status.success());
  assert_eq!(stdout, "");
  assert_eq!(
    stderr,
    indoc! {"
      xc 🏃 (Python 3)
         Found 1 tests from 1 files in <TIME>s
            PASS [   <TIME>s] ./tests/output/passing_test.py test_add
      ------------
         Summary [   <TIME>s] 1 tests run: 1 passed, 0 skipped"}
  );
}

#[test]
fn regular_output_failing() {
  let output = Command::cargo_bin(env!("CARGO_PKG_NAME"))
    .unwrap()
    .arg("./tests/output/failed_test.py")
    .output()
    .unwrap();
  let (stdout, stderr) = (clean_output(output.stdout), clean_output(output.stderr));

  assert!(!output.status.success());
  assert_eq!(stdout, "");
  assert_eq!(
    stderr,
    indoc! {"
      xc 🏃 (Python 3)
         Found 1 tests from 1 files in <TIME>s
            FAIL [   <TIME>s] ./tests/output/failed_test.py test_fails

      FAIL: test_fails (./tests/output/failed_test.py)
      AssertionError:

      ╭─ Traceback:
      │  test_fails (xc/tests/output/failed_test.py:2)
      ╰─"}
  );
}

#[test]
fn regular_output_failing_no_fail_fast() {
  let output = Command::cargo_bin(env!("CARGO_PKG_NAME"))
    .unwrap()
    .arg("./tests/output/failed_test.py")
    .arg("--no-fail-fast")
    .output()
    .unwrap();
  let (stdout, stderr) = (clean_output(output.stdout), clean_output(output.stderr));

  assert!(!output.status.success());
  assert_eq!(stdout, "");
  assert_eq!(
    stderr,
    indoc! {"
      xc 🏃 (Python 3)
         Found 1 tests from 1 files in <TIME>s
            FAIL [   <TIME>s] ./tests/output/failed_test.py test_fails
      ------------
         Summary [   <TIME>s] 1 tests run: 0 passed, 1 failed, 0 skipped

      FAIL: test_fails (./tests/output/failed_test.py)
      AssertionError:

      ╭─ Traceback:
      │  test_fails (xc/tests/output/failed_test.py:2)
      ╰─"}
  );
}

#[test]
fn regular_output_skipped() {
  let output = Command::cargo_bin(env!("CARGO_PKG_NAME"))
    .unwrap()
    .arg("./tests/output/skipped_test.py")
    .output()
    .unwrap();
  let (stdout, stderr) = (clean_output(output.stdout), clean_output(output.stderr));

  assert!(!output.status.success());
  assert_eq!(stdout, "");
  assert_eq!(
    stderr,
    indoc! {"
      xc 🏃 (Python 3)
         Found 1 tests from 1 files in <TIME>s
            SKIP [   <TIME>s] ./tests/output/skipped_test.py test_one
      ------------
         Summary [   <TIME>s] 0 tests run: 0 passed, 1 skipped"}
  );
}

#[test]
fn regular_output_failing_stdout() {
  let output = Command::cargo_bin(env!("CARGO_PKG_NAME"))
    .unwrap()
    .arg("./tests/output/failed_test_stdout.py")
    .output()
    .unwrap();
  let (stdout, stderr) = (clean_output(output.stdout), clean_output(output.stderr));

  assert!(!output.status.success());
  assert_eq!(stdout, "");
  assert_eq!(
    stderr,
    indoc! {"
      xc 🏃 (Python 3)
         Found 1 tests from 1 files in <TIME>s
            FAIL [   <TIME>s] ./tests/output/failed_test_stdout.py test_fails_with_output

      FAIL: test_fails_with_output (./tests/output/failed_test_stdout.py)
      AssertionError:

      ╭─ Traceback:
      │  test_fails_with_output (xc/tests/output/failed_test_stdout.py:3)
      ╰─
      ╭─ Stdout:
      │  Output
      ╰─"}
  );
}

#[test]
fn regular_output_failing_stdout_no_capture() {
  let output = Command::cargo_bin(env!("CARGO_PKG_NAME"))
    .unwrap()
    .arg("./tests/output/failed_test_stdout.py")
    .arg("--no-output-capture")
    .output()
    .unwrap();
  let (stdout, stderr) = (clean_output(output.stdout), clean_output(output.stderr));

  assert!(!output.status.success());
  assert_eq!(stdout, "Output");
  assert_eq!(
    stderr,
    indoc! {"
      xc 🏃 (Python 3)
         Found 1 tests from 1 files in <TIME>s
            FAIL [   <TIME>s] ./tests/output/failed_test_stdout.py test_fails_with_output

      FAIL: test_fails_with_output (./tests/output/failed_test_stdout.py)
      AssertionError:

      ╭─ Traceback:
      │  test_fails_with_output (xc/tests/output/failed_test_stdout.py:3)
      ╰─"}
  );
}

/// Cleans stdout & stderr outputs so it is consistent and readable for tests
/// - Removes ANSI Escape Codes
/// - Replaces Times and Versions with placeholders as they will change
/// - Remove trailing spaces
/// - Removes absolute paths
fn clean_output(string: Vec<u8>) -> String {
  let string = strip_ansi_escapes::strip_str(String::from_utf8(string).unwrap());

  let time_regex = Regex::new("[0-9]+.[0-9]{2,3}s").unwrap();
  let python_version_regex = Regex::new(r"\(Python 3.*").unwrap();
  let windows_paths_regex = Regex::new(r"\(.*\\?:.*xc").unwrap();
  let unix_paths_regex = Regex::new(r"/home.*xc").unwrap();
  let mac_paths_regex = Regex::new(r"/Users.*xc").unwrap();
  let windows_path_separator = Regex::new(r"\\").unwrap();

  let string = time_regex.replace_all(&string, "<TIME>s");
  let string = python_version_regex.replace(&string, "(Python 3)");
  let string = windows_paths_regex.replace_all(&string, "(xc");
  let string = unix_paths_regex.replace_all(&string, "xc");
  let string = mac_paths_regex.replace_all(&string, "xc");
  let string = windows_path_separator.replace_all(&string, "/");

  string
    .lines()
    .map(|line| line.trim_end())
    .collect::<Vec<_>>()
    .join("\n")
}
