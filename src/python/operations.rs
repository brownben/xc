use pyo3_ffi::{self as ffi};
use std::{
  ffi::{CStr, CString},
  fs, io, path,
};

use super::{PyError, PyObject};

/// Represents an interpreter with the GIL held, so we can perform actions on it
pub struct ActiveInterpreter {
  /// Stops other modules from accidentally creating this struct
  _private: (),
}
#[allow(clippy::unused_self, reason = "to ensure GIL is held")]
impl ActiveInterpreter {
  /// SAFETY: Requires the GIL to be held
  pub(crate) unsafe fn new() -> Self {
    Self { _private: () }
  }

  /// Creates a new Python string from a Rust string
  pub fn new_string(&self, string: &str) -> PyObject {
    // SAFETY: String has a valid length, and the pointer is valid
    let length = string.len().try_into().unwrap();
    let result = unsafe { ffi::PyUnicode_FromStringAndSize(string.as_ptr().cast(), length) };

    unsafe { PyObject::from_ptr_unchecked(result) }
  }

  /// Imports a module
  ///
  /// SAFETY: Assumes that the module exists
  pub fn import_module(&self, module: &CStr) -> PyObject {
    let result = unsafe { ffi::PyImport_ImportModule(module.as_ptr().cast()) };

    debug_assert!(!result.is_null());

    let object = unsafe { PyObject::from_ptr_unchecked(result) };
    debug_assert!(object.is_module());
    object
  }

  /// Redirect stdout and stderr from Python into a string
  ///
  /// Captured output can be fetched by [`Self::get_captured_output`]
  pub fn capture_output(&self) {
    let sys = self.import_module(c"sys");
    let io = self.import_module(c"io");

    let string_io = io.get_attr(&self.new_string("StringIO")).unwrap();
    let stdout_io = unsafe { string_io.call_unchecked() };
    let stderr_io = unsafe { string_io.call_unchecked() };

    _ = sys.set_attr(&self.new_string("stdout"), stdout_io);
    _ = sys.set_attr(&self.new_string("stderr"), stderr_io);
  }

  /// Get the captured stdout and stderr
  pub fn get_captured_output(&self) -> (Option<String>, Option<String>) {
    let sys = self.import_module(c"sys");

    let stdout = sys.get_attr(&self.new_string("stdout")).unwrap();
    let stderr = sys.get_attr(&self.new_string("stderr")).unwrap();

    let get_value_str = self.new_string("getvalue");

    // The user may have altered stdout/ stderr, or captured output may not be enabled
    let stdout_value = stdout
      .get_attr(&get_value_str)
      .and_then(|value| value.call())
      .map(|x| x.to_string())
      .ok();
    let stderr_value = stderr
      .get_attr(&get_value_str)
      .and_then(|value| value.call())
      .map(|x| x.to_string())
      .ok();

    (stdout_value, stderr_value)
  }

  /// Adds the given path to Python's module resolution path variable.
  ///
  /// Most commonly used to add the current folder to the module search path.
  /// Assumes Python Interpreter is currently active.
  pub fn add_to_sys_modules_path(&self, path: &CStr) {
    let sys = self.import_module(c"sys");
    let path_list = sys.get_attr(&self.new_string("path")).unwrap();

    unsafe {
      let path_string = ffi::PyUnicode_FromString(path.as_ptr());
      ffi::PyList_Insert(path_list.as_ptr(), 0, path_string);
    }
  }

  /// Adds the lowest parent path which is not  python module to the module search path.
  pub fn add_parent_module_to_path(&self, file: &path::Path) {
    if let Some(parent) = file.parent() {
      if is_python_module(parent) {
        self.add_parent_module_to_path(parent);
      } else {
        self.add_to_sys_modules_path(&path_to_cstring(parent));
      }
    }
  }

  /// Compile the given file into a [`PyObject`] containing the bytecode
  pub fn compile_file(&self, file: &path::Path) -> Result<PyObject, PyError> {
    let file = &file.canonicalize().expect("file to exist");

    let file_name = filename_to_cstring(file);
    let source = read_file_to_cstring(file).unwrap();

    let code_object =
      unsafe { ffi::Py_CompileString(source.as_ptr(), file_name.as_ptr(), ffi::Py_file_input) };

    PyObject::from_ptr_or_error(code_object)
  }

  /// Execute the given and returns the [`PyObject`] representing the executed module, or an error which has arisen
  pub fn execute_file(&self, file: &path::Path) -> Result<PyObject, PyError> {
    let file_name = filename_to_cstring(file);
    let module_name = calculate_module_name(file);
    let bytecode_object = self.compile_file(file)?;

    // Execute the compiled bytecode module
    let module = unsafe {
      ffi::PyImport_ExecCodeModuleEx(
        module_name.as_ptr(),
        bytecode_object.as_ptr(),
        file_name.as_ptr(),
      )
    };

    PyObject::from_ptr_or_error(module)
  }

  /// Execute a string of python code as a module
  pub(crate) fn execute_string(&self, string: &CStr) -> Result<PyObject, PyError> {
    let file_name = c"<xc internals>";

    let bytecode_object_ptr =
      unsafe { ffi::Py_CompileString(string.as_ptr(), file_name.as_ptr(), ffi::Py_file_input) };
    let bytecode_object = PyObject::from_ptr_or_error(bytecode_object_ptr)?;
    debug_assert!(bytecode_object.is_code_object());

    // Execute the compiled bytecode module
    let module = unsafe {
      ffi::PyImport_ExecCodeModuleEx(
        file_name.as_ptr(),
        bytecode_object.as_ptr(),
        file_name.as_ptr(),
      )
    };

    PyObject::from_ptr_or_error(module)
  }
}

/// Read the entire contents of a file into a [`CString`].
///
/// Returns an error if the given file doesn't exist.
fn read_file_to_cstring(path: &path::Path) -> io::Result<CString> {
  let buffer = fs::read(path)?;

  Ok(unsafe { CString::from_vec_unchecked(buffer) })
}

fn filename_to_cstring(path: &path::Path) -> CString {
  let file_name = path.to_string_lossy();

  CString::new(file_name.to_string()).unwrap()
}

fn path_to_cstring(path: &path::Path) -> CString {
  let file_name = path.to_string_lossy();

  CString::new(file_name.to_string()).unwrap()
}

/// Calculates the module name of a file.
///
/// Traverses up directories checking if they are Python modules,
/// to generate the module name
///
///
/// For example for the below `test.py` file the module name would be `birds.eggs.test`.
///
/// ```md
/// birds
/// ├─ __init__.py
/// ╰─ eggs/
///    ├─ __init__.py
///    ╰─ test.py
/// ```
fn calculate_module_name(file: &path::Path) -> CString {
  fn calculate_module_name_inner(folder: &path::Path, module_name: &mut String) {
    if !is_python_module(folder) {
      return;
    }

    if let Some(parent) = folder.parent() {
      calculate_module_name_inner(parent, module_name);
    }

    if !module_name.is_empty() {
      module_name.push('.');
    }

    let filename = &folder.file_stem().unwrap().to_str().unwrap();

    module_name.push_str(filename);
  }

  let file: &path::Path = &file.canonicalize().expect("file to exist");
  let mut module_name = String::new();

  calculate_module_name_inner(file, &mut module_name);

  CString::new(module_name).unwrap()
}

/// Checks if a file is a Python module.
///
/// It is a module if it directory containing a __init__.py file, or a Python file
fn is_python_module(file: &path::Path) -> bool {
  if !file.is_dir() {
    return file.extension().unwrap_or_default() == "py";
  }

  let mut init_file = file.to_path_buf();
  init_file.push("./__init__.py");

  init_file.exists()
}
