use assert_cmd::Command;
use std::env;

fn count_tests_run(args: &[&str]) -> usize {
  let cmd_output = Command::cargo_bin(env!("CARGO_PKG_NAME"))
    .unwrap()
    .args(args)
    .arg("--output=json")
    .output()
    .unwrap();

  let stdout = String::from_utf8(cmd_output.stdout).unwrap();
  stdout.lines().count()
}

#[test]
fn find_all_tests_in_folder() {
  assert_eq!(count_tests_run(&["./tests/discovery/"]), 6);
}

#[test]
fn non_existant_paths() {
  assert_eq!(count_tests_run(&["./discovery/tests/"]), 0);
  assert_eq!(count_tests_run(&["./discovery/tests/x.py"]), 0);
}

#[test]
fn files_run_individually() {
  assert_eq!(count_tests_run(&["./tests/discovery/a.py"]), 1);
  assert_eq!(count_tests_run(&["./tests/discovery/b.py"]), 2);
  assert_eq!(count_tests_run(&["./tests/discovery/c.py"]), 3);
}

#[test]
fn exclude_single_file() {
  let base_path = "./tests/discovery";
  for (file, test_count) in [("a", 5), ("b", 4), ("c", 3)] {
    let relative = format!("--exclude=./tests/discovery/{file}.py");
    let path = format!("--exclude=tests/discovery/{file}.py");
    let glob_file = format!("--exclude=**/{file}.py");
    let file = format!("--exclude={file}.py");

    assert_eq!(count_tests_run(&[base_path, &relative]), test_count);
    assert_eq!(count_tests_run(&[base_path, &path]), test_count);
    assert_eq!(count_tests_run(&[base_path, &glob_file]), test_count);
    assert_eq!(count_tests_run(&[base_path, &file]), test_count);
  }
}

#[test]
fn file_with_invalid_syntax() {
  assert_eq!(count_tests_run(&["./tests/discovery/invalid_syntax.py"]), 0);
}
