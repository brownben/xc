//! Discover tests in Python files so they can be run later

use std::{
  fs,
  num::NonZero,
  path::{Path, PathBuf},
  sync::Mutex,
  thread,
  time::{Duration, Instant},
};

/// Represents a discovered test in a Python file
#[derive(Debug, Clone)]
pub enum Test {
  /// A test which is a function
  Function { file: PathBuf, function: String },

  /// A test which is a method on a class
  Method {
    file: PathBuf,
    class: String,
    method: String,
  },
}
impl Test {
  /// Get the file of the test
  pub fn file(&self) -> &Path {
    match self {
      Test::Function { file, .. } | Test::Method { file, .. } => file,
    }
  }

  /// Get the suite of the test
  pub fn suite(&self) -> Option<&str> {
    match self {
      Test::Function { .. } => None,
      Test::Method { class, .. } => Some(class),
    }
  }

  /// Get the name of the test function/ method
  pub fn name(&self) -> &str {
    match self {
      Test::Function { function, .. } => function,
      Test::Method { method, .. } => method,
    }
  }

  /// Get the name and suite of the test combined into a single identifier
  pub fn identifier(&self) -> String {
    let mut identifier = String::new();
    if let Some(suite) = self.suite() {
      identifier.push_str(suite);
      identifier.push('.');
    }
    identifier.push_str(self.name());
    identifier
  }
}

/// Holds the tests discovered, and metadata about the discovery
pub struct DiscoveredTests {
  start: Instant,
  /// How long it took to find the tests
  pub duration: Duration,
  /// The tests that were found
  pub tests: Vec<Test>,
  /// How many tests were found
  pub test_count: usize,
  /// How many files were the tests found in
  pub file_count: usize,
}
impl DiscoveredTests {
  fn new() -> Self {
    Self {
      start: Instant::now(),
      duration: Duration::from_secs(0),
      tests: Vec::new(),
      file_count: 0,
      test_count: 0,
    }
  }
}

/// Given a set of initial paths to search, get all the tests in those files.
///
/// Files are searched in parallel, and respect .gitignore
pub fn find_tests(paths: &[PathBuf]) -> DiscoveredTests {
  let (first_path, rest_paths) = paths.split_first().expect("at least one path to search");

  // Create the `WalkBuilder`.
  let mut builder = ignore::WalkBuilder::new(first_path);
  for path in rest_paths {
    builder.add(path);
  }
  builder.standard_filters(true);
  builder.threads(
    thread::available_parallelism()
      .map(NonZero::get)
      .unwrap_or(1),
  );

  let state: Mutex<DiscoveredTests> = Mutex::new(DiscoveredTests::new());
  let mut local_file_builder = TestFinderBuilder { global: &state };
  let walker = builder.build_parallel();
  walker.visit(&mut local_file_builder);

  state.into_inner().unwrap()
}

/// Find any tests in a given file.
///
/// - Parse the file as Python
/// - Find any functions or methods which match the signature for a test
/// - Record those tests
fn get_test_methods(file: &Path, tests: &mut Vec<Test>) {
  let Ok(source) = fs::read_to_string(file) else {
    return;
  };
  let Ok(ast) = ruff_python_parser::parse_module(&source) else {
    return;
  };

  for statement in &ast.syntax().body {
    if let Some(function_def) = statement.as_function_def_stmt() {
      let name = &function_def.name;

      if name.starts_with("test") {
        tests.push(Test::Function {
          file: file.into(),
          function: name.to_string(),
        });
      }
    }

    if let Some(class_def) = statement.as_class_def_stmt() {
      let name = &class_def.name;

      if name.starts_with("Test") || name.ends_with("Test") || name.ends_with("Tests") {
        for statement in &class_def.body {
          if let Some(method) = statement.as_function_def_stmt() {
            if method.name.starts_with("test") {
              tests.push(Test::Method {
                file: file.into(),
                class: name.to_string(),
                method: method.name.to_string(),
              });
            }
          }
        }
      }
    }
  }
}

struct TestFinderBuilder<'a> {
  global: &'a Mutex<DiscoveredTests>,
}
impl<'a> ignore::ParallelVisitorBuilder<'a> for TestFinderBuilder<'a> {
  fn build(&mut self) -> Box<dyn ignore::ParallelVisitor + 'a> {
    Box::new(TestFinder {
      global: self.global,
      file_count: 0,
      tests: Vec::new(),
    })
  }
}

struct TestFinder<'a> {
  global: &'a Mutex<DiscoveredTests>,
  file_count: usize,
  tests: Vec<Test>,
}
impl ignore::ParallelVisitor for TestFinder<'_> {
  fn visit(&mut self, entry: Result<ignore::DirEntry, ignore::Error>) -> ignore::WalkState {
    if let Ok(path) = entry {
      let path = path.path();

      if path.is_file() && path.extension().unwrap_or_default() == "py" {
        self.file_count += 1;
        get_test_methods(path, &mut self.tests);
      }
    }

    ignore::WalkState::Continue
  }
}
impl Drop for TestFinder<'_> {
  fn drop(&mut self) {
    let mut global_state = self.global.lock().unwrap();
    global_state.tests.extend_from_slice(&self.tests);
    global_state.file_count += self.file_count;
    global_state.duration = global_state.start.elapsed();
    global_state.test_count += self.tests.len();
  }
}
