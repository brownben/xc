//! # Coverage
//! Track which lines of code have been executed.

use rayon::prelude::*;
use std::collections::{BTreeSet, HashMap};
use std::ffi::CString;
use std::{fs, path};

use crate::python::{
  objects::{PyDict, PyError, PyIter, PyObject, PyTuple},
  ActiveInterpreter, Interpreter, MainInterpreter, SubInterpreter,
};

/// Enables line coverage collection, and returns the tracer object
pub fn enable_collection(python: &ActiveInterpreter) -> PyObject {
  let raw_source = include_str!("./monitoring.py");
  let source = CString::new(raw_source).unwrap();
  let module = python.execute_string(&source).expect("code to run");

  module
    .get_attr(&python.new_string("tracer"))
    .expect("`tracer` var to exist")
}

/// Get the lines that have been executed, converting them from a Python structure
pub fn get_executed_lines(_python: &ActiveInterpreter, tracer_object: &PyObject) -> Lines {
  let lines = tracer_object.get_attr_cstr(c"lines").unwrap();
  let filename_line_pairs = PyDict::from_object(lines).unwrap().items();

  filename_line_pairs
    .into_iter()
    .map(|tuple| {
      let tuple = unsafe { PyTuple::from_object_unchecked(tuple) };
      let filename = unsafe { tuple.get_item_unchecked(0).to_string() };
      let lines_set = unsafe { tuple.get_item_unchecked(1) };

      let lines = lines_set
        .into_iter()
        .map(|line_no| line_no.as_long())
        .collect();

      (filename, lines)
    })
    .collect()
}

/// Holds the line numbers for a file
///
/// Either which have been executed, or which line numbers are reachable
#[derive(Clone, Debug, Default)]
pub struct Lines(HashMap<String, BTreeSet<i32>>);
impl Lines {
  pub fn get_lines(&self, file_name: &str) -> Option<&BTreeSet<i32>> {
    let full_filename = fs::canonicalize(file_name).unwrap();

    self.0.get(full_filename.to_str().unwrap())
  }

  pub fn iter(&self) -> impl Iterator<Item = (&String, &BTreeSet<i32>)> {
    self.0.iter()
  }
}
// Merge together the executed line information
impl ParallelExtend<Option<Lines>> for Lines {
  fn par_extend<I>(&mut self, iter: I)
  where
    I: IntoParallelIterator<Item = Option<Lines>>,
  {
    let items: Vec<(_, _)> = iter
      .into_par_iter()
      .filter(Option::is_some)
      .flat_map_iter(|cov| cov.unwrap().0.into_iter())
      .collect();

    for (file, lines) in items {
      self.0.entry(file).or_default().extend(lines);
    }
  }
}
// Merge together file stats from discovering possible lines
impl FromParallelIterator<Option<(String, BTreeSet<i32>)>> for Lines {
  fn from_par_iter<I>(iter: I) -> Self
  where
    I: IntoParallelIterator<Item = Option<(String, BTreeSet<i32>)>>,
  {
    let mut collection = Lines::default();
    collection.0.par_extend(iter.into_par_iter().flatten());
    collection
  }
}
impl FromIterator<(String, BTreeSet<i32>)> for Lines {
  fn from_iter<I>(iter: I) -> Self
  where
    I: IntoIterator<Item = (String, BTreeSet<i32>)>,
  {
    let mut collection = Lines::default();
    collection.0.extend(iter);
    collection
  }
}

/// Get all the lines of Python which could be reported as run in the given paths.
///
/// The compiled bytecode contains this information. Empty lines for example will
/// not be reported but are still executed. So we need to find which lines could be run,
/// rather than just the range of the file.
pub fn get_executable_lines(
  interpreter: &MainInterpreter,
  coverage_include: &[path::PathBuf],
  coverage_exclude: &[path::PathBuf],
) -> Lines {
  let (first_path, rest_paths) = coverage_include
    .split_first()
    .expect("at least one path to search");

  // Create the `WalkBuilder`, respecting .gitignore
  let mut builder = ignore::WalkBuilder::new(first_path);
  builder.standard_filters(true);

  // Only look for Python files
  let mut types = ignore::types::TypesBuilder::new();
  types.add("python", "*.py").unwrap();
  types.select("python");
  builder.types(types.build().unwrap());

  // Add the paths to search
  for path in rest_paths {
    builder.add(path);
  }

  // Exclude specified from the search
  let mut exclude_override = ignore::overrides::OverrideBuilder::new("");
  for path in coverage_exclude {
    exclude_override
      .add(&format!("!{}", path.to_string_lossy()))
      .unwrap();
  }
  builder.overrides(exclude_override.build().unwrap());

  // Run the search
  builder
    .build()
    .par_bridge()
    .map(|path| {
      let path = path.ok()?.into_path();

      if !path.is_file() {
        return None;
      }

      // as we are compiling python, we need an interpreter
      let line_numbers = SubInterpreter::new(interpreter)
        .with_gil(|python| get_line_numbers_for_python_file(python, &path));

      if line_numbers.is_empty() {
        None
      } else {
        let file_name = path.to_string_lossy().into_owned();
        Some((file_name, line_numbers))
      }
    })
    .collect()
}

fn get_line_numbers_for_python_file(
  python: &ActiveInterpreter,
  path: &path::Path,
) -> BTreeSet<i32> {
  let mut line_numbers = BTreeSet::new();
  let code_object = python.compile_file(path).unwrap();
  let _ = get_line_numbers_from_code_object(python, &code_object, &mut line_numbers);

  line_numbers
}

fn get_line_numbers_from_code_object(
  python: &ActiveInterpreter,
  code_object: &PyObject,
  line_numbers: &mut BTreeSet<i32>,
) -> Result<(), PyError> {
  // Search all constants for code objects and recurse into them
  let constants = code_object.get_attr(&python.new_string("co_consts"))?;
  let code_objects = constants.into_iter().filter(PyObject::is_code_object);
  for code_object in code_objects {
    get_line_numbers_from_code_object(python, &code_object, line_numbers)?;
  }

  // Get all the line numbers from the code object
  // The `co_lines` function is specified here: https://peps.python.org/pep-0626/
  // Gives a tuple of (bytecode_start_byte, bytecode_end_byte, line_number)
  let iterator_lines = unsafe {
    code_object
      .get_attr_unchecked(&python.new_string("co_lines"))
      .call_unchecked()
  };
  line_numbers.extend(
    unsafe { PyIter::from_object_unchecked(iterator_lines) }
      .map(|line_tuple| unsafe { PyTuple::from_object_unchecked(line_tuple) })
      .filter_map(|tuple| {
        let line_number = unsafe { tuple.get_item_unchecked(2) };
        line_number.is_number().then(|| line_number.as_long())
      })
      .filter(|line_number| *line_number > 0),
  );

  Ok(())
}

pub fn print_summary(possible: &Lines, executed: &Lines) {
  use anstream::eprintln;
  use owo_colors::OwoColorize;

  let empty = BTreeSet::new();

  eprintln!("\n{}{}", "╭─ ".dimmed(), "Coverage".bold());
  eprintln!(
    "{}{:55} {}",
    "│  ".dimmed(),
    "File".dimmed().italic(),
    "Lines    Missed  Coverage".dimmed().italic(),
  );

  for (file_name, possible_lines) in possible.iter() {
    let executed_lines = executed.get_lines(file_name).unwrap_or(&empty);
    let covered_lines = possible_lines.intersection(executed_lines).count();
    let total_lines = possible_lines.len();
    let missed_lines = total_lines - covered_lines;

    #[expect(clippy::cast_precision_loss, reason = "line numbers < f64::MAX")]
    let coverage = (covered_lines as f64 / total_lines as f64) * 100.0;

    eprintln!(
      "{}{:55}{:6}{:>10}{:>9.1}%",
      "├─ ".dimmed(),
      file_name,
      total_lines,
      missed_lines,
      coverage,
    );
  }
  eprintln!("{}", "╰──".dimmed());
}
