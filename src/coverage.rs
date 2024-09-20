//! # Coverage
//! Track which lines of code have been executed.
#![allow(unsafe_code)]

use pyo3_ffi::{self as ffi};
use rayon::prelude::*;
use std::collections::{BTreeSet, HashMap};
use std::ffi::CString;
use std::{fs, path};

use crate::python::{self, PyObject};

/// Enables line coverage collection, and returns the tracer object
pub fn enable_collection() -> PyObject {
  let raw_source = include_str!("./monitoring.py");
  let source = CString::new(raw_source).unwrap();
  let module = python::execute_string(&source).expect("code to run");

  module.get_attr_cstr(c"tracer").expect("var to exist")
}

/// Get the lines that have been executed, converting them from a Python structure
pub fn get_executed_lines(tracer_object: &PyObject) -> Lines {
  unsafe {
    let lines = tracer_object.get_attr_cstr(c"lines").unwrap();
    let filename_line_pairs = PyObject::new(ffi::PyDict_Items(lines.as_ptr())).unwrap();

    filename_line_pairs
      .into_iter()
      .map(|tuple| {
        let filename = tuple.get_tuple_item(0).unwrap().to_string();
        let lines = tuple
          .get_tuple_item(1)
          .unwrap()
          .into_iter()
          .map(PyObject::get_long)
          .collect();

        (filename, lines)
      })
      .collect()
  }
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
pub fn get_executable_lines(paths: &[path::PathBuf]) -> Lines {
  let (first_path, rest_paths) = paths.split_first().expect("at least one path to search");

  // Create the `WalkBuilder`.
  let mut builder = ignore::WalkBuilder::new(first_path);
  for path in rest_paths {
    builder.add(path);
  }
  builder.standard_filters(true);

  builder
    .build()
    .par_bridge()
    .map(|path| {
      let path = path.ok()?.into_path();

      if !path.is_file() || path.extension().unwrap_or_default() != "py" {
        return None;
      }

      // as we are compiling python, we need an interpreter
      let line_numbers =
        python::SubInterpreter::new().run(|| get_line_numbers_for_python_file(&path));

      if line_numbers.is_empty() {
        None
      } else {
        let file_name = path.to_string_lossy().into_owned();
        Some((file_name, line_numbers))
      }
    })
    .collect()
}

fn get_line_numbers_for_python_file(path: &path::Path) -> BTreeSet<i32> {
  let mut line_numbers = BTreeSet::new();
  let code_object = crate::python::compile_file(path).unwrap();
  let _ = get_line_numbers_from_code_object(&code_object, &mut line_numbers);

  line_numbers
}

fn get_line_numbers_from_code_object(
  code_object: &PyObject,
  line_numbers: &mut BTreeSet<i32>,
) -> Result<(), python::Error> {
  // Search all constants for code objects and recurse into them
  let constants = code_object.get_attr_cstr(c"co_consts")?;
  let code_objects = constants.into_iter().filter(PyObject::is_code_object);
  for code_object in code_objects {
    get_line_numbers_from_code_object(&code_object, line_numbers)?;
  }

  // Get all the line numbers from the code object
  // The `co_lines` function is specified here: https://peps.python.org/pep-0626/
  // Gives a tuple of (bytecode_start_byte, bytecode_end_byte, line_number)
  let iterator_lines = code_object.get_attr_cstr(c"co_lines")?.call()?;
  line_numbers.extend(
    iterator_lines
      .iter()
      .map(|line_tuple| line_tuple.get_tuple_item(2).unwrap().get_long())
      .filter(|line_number| *line_number > 0),
  );

  Ok(())
}
