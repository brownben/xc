//! # Coverage
//! Track which lines of code have been executed.
#![allow(unsafe_code)]

use pyo3_ffi::{self as ffi};
use rayon::prelude::*;
use std::collections::{BTreeSet, HashMap};
use std::{fs, mem, path, ptr};

use crate::python::{self, PyObject};

/// Object that is used by the trace function to track coverage and store intermediate state
#[repr(C)]
#[derive(Debug)]
pub struct TraceObject {
  /// Header to be viewed as a Python object
  py: ffi::PyObject,

  /// Do we expect the next line to be in the same function?
  possible_function_change: bool,

  /// A pointer to the current file name, so we can quickly check if it has changed
  current_file_pointer: *mut ffi::PyObject,
  /// The current file name, and the lines that have been executed
  current: Option<(String, BTreeSet<i32>)>,

  /// All the lines that have been executed
  all_files: Lines,

  /// The path to the standard library, so we can ignore it
  standard_library_path: String,
}
impl TraceObject {
  pub fn new(standard_library_path: String) -> Self {
    Self {
      py: ffi::PyObject_HEAD_INIT,

      possible_function_change: false,
      current_file_pointer: ptr::null_mut(),
      current: None,
      all_files: Lines::default(),

      standard_library_path,
    }
  }

  fn save_current_file(&mut self) {
    if let Some((filename, lines)) = mem::take(&mut self.current) {
      if !lines.is_empty() {
        self.all_files.add_file(filename, lines);
      }
    }
  }

  fn should_trace_file(&self, file_name: &str) -> bool {
    let is_real_file = !file_name.starts_with('<'); // real files don't start with <
    let is_not_stdlib = !file_name.starts_with(&self.standard_library_path);

    is_real_file && is_not_stdlib
  }

  pub fn finish(mut self) -> Lines {
    if let Some((file_name, lines)) = self.current {
      if !lines.is_empty() {
        self.all_files.add_file(file_name, lines);
      }
    }

    self.all_files
  }
}

pub unsafe extern "C" fn trace_function(
  obj: *mut ffi::PyObject,
  frame: *mut ffi::PyFrameObject,
  what: std::ffi::c_int,
  _arg: *mut ffi::PyObject,
) -> std::ffi::c_int {
  let coverage: &mut TraceObject = &mut *obj.cast();

  // Check if we need to change which file we are recording into?
  if coverage.possible_function_change {
    let code_object = &*ffi::PyFrame_GetCode(frame);

    // Has the filename actually changed?
    if coverage.current_file_pointer != code_object.co_filename {
      coverage.save_current_file();

      // Decide whether to trace a file
      let filename = PyObject::new(code_object.co_filename).unwrap().to_string();
      coverage.current = if coverage.should_trace_file(&filename) {
        // Load existing information for the file
        let lines = coverage.all_files.take_existing_lines(&filename);
        Some((filename, lines))
      } else {
        None
      };
    }

    coverage.current_file_pointer = code_object.co_filename;
    coverage.possible_function_change = false;
  }

  match what {
    ffi::PyTrace_CALL | ffi::PyTrace_RETURN | ffi::PyTrace_EXCEPTION => {
      // we can only change file if we go in or out of a function
      coverage.possible_function_change = true;
    }
    ffi::PyTrace_LINE => {
      // if we are tracing this file, record the line number
      if let Some((ref _filename, ref mut lines)) = coverage.current {
        let line_number = ffi::PyFrame_GetLineNumber(frame);
        lines.insert(line_number);
      }
    }
    _ => {}
  }

  0
}

/// Holds the line numbers for a file
///
/// Either which have been executed, or which line numbers are reachable
#[derive(Clone, Debug, Default)]
pub struct Lines(HashMap<String, BTreeSet<i32>>);
impl Lines {
  fn add_file(&mut self, file_name: String, line_numbers: BTreeSet<i32>) {
    self.0.insert(file_name, line_numbers);
  }

  fn take_existing_lines(&mut self, file_name: &str) -> BTreeSet<i32> {
    self.0.get_mut(file_name).map(mem::take).unwrap_or_default()
  }

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
  let constants = code_object.get_attr_cstr(c"co_consts")?.as_iterator()?;
  let code_objects = constants.iter().filter(PyObject::is_code_object);
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
      .map(|line_tuple| unsafe {
        let line_number_object = ffi::PyTuple_GET_ITEM(line_tuple.as_ptr(), 2);
        ffi::PyLong_AsLong(line_number_object)
      })
      .filter(|line_number| *line_number > 0),
  );

  Ok(())
}
