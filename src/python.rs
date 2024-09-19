//! Interface for creating subinterpreters and running python code
//!
//! Py03 doesn't yet support subinterpreters, so we have to use the
//! C API directly. It is a very basic interface over the C API, and is
//! not intended for other uses, as it makes many assumptions.
#![allow(unsafe_code)]

use pyo3_ffi::{self as ffi};
use serde::{Deserialize, Serialize};
use std::ffi::{CStr, CString};
use std::{env, fmt, fs, io, iter, mem, path, ptr};
use widestring::WideCString;

use crate::coverage;

pub fn version() -> String {
  let version = unsafe { CStr::from_ptr(ffi::Py_GetVersion()) };
  let platform = unsafe { CStr::from_ptr(ffi::Py_GetPlatform()) };

  let mut output = String::from(version.to_string_lossy());
  output.push(' ');
  output.push_str(&platform.to_string_lossy());
  output
}

/// Represents the main Python Interpreter
///
/// Initialise's the interpreter on creation, and stores the thread state so
/// it can be cleaned up correctly on drop.
///
/// SAFETY: It must exist for any operations with Python to work
pub struct Interpreter {
  main_thread_state: *mut ffi::PyThreadState,
  _virtual_enviroment_path: WideCString,
}
impl Interpreter {
  pub fn initialize() -> Self {
    let mut config: mem::MaybeUninit<ffi::PyConfig> = mem::MaybeUninit::uninit();
    let mut virtual_enviroment_path: WideCString = WideCString::new();

    unsafe {
      ffi::PyConfig_InitPythonConfig(ptr::from_mut(config.assume_init_mut()));

      if let Ok(virtual_enviroment) = env::var("VIRTUAL_ENV") {
        virtual_enviroment_path = WideCString::from_str(&virtual_enviroment).unwrap();
        config.assume_init_mut().prefix = virtual_enviroment_path.as_mut_ptr();
      }

      ffi::Py_InitializeFromConfig(ptr::from_mut(config.assume_init_mut()));
    }

    let main_thread_state = unsafe { ffi::PyThreadState_Swap(ptr::null_mut()) };

    Self {
      main_thread_state,
      _virtual_enviroment_path: virtual_enviroment_path,
    }
  }
}
impl Drop for Interpreter {
  fn drop(&mut self) {
    unsafe { ffi::PyEval_RestoreThread(self.main_thread_state) };
    unsafe { ffi::Py_FinalizeEx() };
  }
}

/// Represents a Python Subinterpreter (An interpreter for a specific thread)
pub struct SubInterpreter {
  interpreter_state: *mut ffi::PyThreadState,
  coverage: Option<coverage::TraceObject>,
}
impl SubInterpreter {
  /// The default configuration to create a Subinterpreter with it's own global interpreter lock
  const DEFAULT_CONFIG: ffi::PyInterpreterConfig = ffi::PyInterpreterConfig {
    use_main_obmalloc: 0,
    allow_fork: 0,
    allow_exec: 0,
    allow_threads: 1,
    allow_daemon_threads: 0,
    check_multi_interp_extensions: 1,
    gil: ffi::PyInterpreterConfig_OWN_GIL,
  };

  /// Creates a new subinterpreter
  ///
  /// SAFETY: The main interpreter must have been initialized
  pub fn new() -> Self {
    let mut interpreter_state = std::ptr::null_mut();

    unsafe {
      ffi::Py_NewInterpreterFromConfig(&mut interpreter_state, &Self::DEFAULT_CONFIG);
      ffi::PyEval_SaveThread(); // Stops Deadlock
    };

    Self {
      interpreter_state,
      coverage: None,
    }
  }

  pub fn enable_coverage(&mut self) {
    unsafe { ffi::PyEval_RestoreThread(self.interpreter_state) };

    let stdlib_path = get_standard_library_path();
    self.coverage = Some(coverage::TraceObject::new(stdlib_path));

    let coverage = ptr::from_mut(self.coverage.as_mut().unwrap()).cast();
    unsafe { ffi::PyEval_SetTrace(Some(coverage::trace_function), coverage) };

    self.interpreter_state = unsafe { ffi::PyEval_SaveThread() };
  }

  pub fn get_coverage(&mut self) -> Option<coverage::Lines> {
    let coverage = mem::take(&mut self.coverage);
    coverage.map(coverage::TraceObject::finish)
  }

  /// Loads the subinterpreter into the current thread, runs the given function,
  /// then destroys the subinterpreter.
  pub fn run<T>(&mut self, f: impl Fn() -> T) -> T {
    unsafe { ffi::PyEval_RestoreThread(self.interpreter_state) };
    let result = f();
    self.interpreter_state = unsafe { ffi::PyEval_SaveThread() };

    result
  }
}
impl Drop for SubInterpreter {
  fn drop(&mut self) {
    unsafe { ffi::PyEval_RestoreThread(self.interpreter_state) };
    unsafe { ffi::Py_EndInterpreter(self.interpreter_state) };
  }
}

/// A pointer to a generic Python Object
pub struct PyObject(ptr::NonNull<ffi::PyObject>);
impl PyObject {
  /// Wraps a raw pointer to a Python Object.
  /// If it is null, an exception has been raised so fetch the exception
  pub fn new(obj: *mut ffi::PyObject) -> Result<PyObject, Error> {
    if let Some(pointer) = ptr::NonNull::new(obj) {
      Ok(PyObject(pointer))
    } else {
      Err(Error::get_exception())
    }
  }

  pub fn as_ptr(&self) -> *mut ffi::PyObject {
    self.0.as_ptr()
  }
  fn as_usize(&self) -> usize {
    unsafe { ffi::PyLong_AsLongLong(self.as_ptr()) }
      .try_into()
      .unwrap()
  }

  /// Get the attribute of the object
  pub fn get_attr(&self, attribute_name: &str) -> Result<PyObject, Error> {
    let attribute_name = CString::new(attribute_name).unwrap();
    self.get_attr_cstr(&attribute_name)
  }
  pub fn get_attr_cstr(&self, attribute_name: &CStr) -> Result<PyObject, Error> {
    let attribute_value =
      unsafe { ffi::PyObject_GetAttrString(self.as_ptr(), attribute_name.as_ptr()) };

    Self::new(attribute_value)
  }

  /// Check if the object has an attribute
  pub fn has_attr(&self, attr: &CStr) -> bool {
    unsafe { ffi::PyObject_HasAttrString(self.as_ptr(), attr.as_ptr()) == 1 }
  }

  /// Check's if given attribute exists, and if it is set to a truthy value
  pub fn has_truthy_attr(&self, attr: &CStr) -> bool {
    let has_attr = self.has_attr(attr);

    if has_attr {
      unsafe {
        let attr = ffi::PyObject_GetAttrString(self.as_ptr(), attr.as_ptr());
        ffi::PyObject_IsTrue(attr) == 1
      }
    } else {
      false
    }
  }

  /// Call the given Object with no arguments
  pub fn call(self) -> Result<PyObject, Error> {
    Self::new(unsafe { ffi::PyObject_CallNoArgs(self.as_ptr()) })
  }

  fn is_none(&self) -> bool {
    unsafe { ffi::Py_IsNone(self.as_ptr()) == 1 }
  }

  fn type_name(&self) -> String {
    unsafe {
      let object_type = ffi::Py_TYPE(self.as_ptr());
      let name_type = Self::new(ffi::PyType_GetName(object_type)).unwrap();

      name_type.to_string()
    }
  }

  /// View as an iterator, and iterate over it
  pub fn iter(&self) -> impl Iterator<Item = PyObject> {
    let iterator = self.as_ptr();

    iter::from_fn(move || {
      let next = unsafe { ffi::PyIter_Next(iterator) };
      if next.is_null() {
        None
      } else {
        Some(PyObject::new(next).unwrap())
      }
    })
  }

  /// Convert the object to an iterator
  pub fn as_iterator(&self) -> Result<PyObject, Error> {
    unsafe { Self::new(ffi::PyObject_GetIter(self.as_ptr())) }
  }

  /// Is the object a code object?
  pub fn is_code_object(&self) -> bool {
    unsafe { ffi::PyCode_Check(self.as_ptr()) == 1 }
  }
}
impl Drop for PyObject {
  fn drop(&mut self) {
    unsafe { ffi::Py_DECREF(self.as_ptr()) };
  }
}
impl fmt::Display for PyObject {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    unsafe {
      let string_object = ffi::PyObject_Str(self.as_ptr());

      let mut size = 0;
      let pointer = ffi::PyUnicode_AsUTF8AndSize(string_object, &mut size);

      let slice = std::slice::from_raw_parts(
        pointer.cast::<u8>(),
        usize::try_from(size).unwrap_unchecked(),
      );
      let str = std::str::from_utf8_unchecked(slice);

      f.write_str(str)
    }
  }
}

#[allow(unused)]
/// An error which occurred whilst executing Python code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Error {
  pub kind: String,
  pub message: String,

  pub traceback: Option<Trackback>,

  pub stdout: String,
  pub stderr: String,
}
impl Error {
  fn get_exception() -> Self {
    let exception_object =
      PyObject(unsafe { ptr::NonNull::new_unchecked(ffi::PyErr_GetRaisedException()) });

    let kind = exception_object.type_name();
    let message = exception_object.to_string();
    let traceback = Trackback::from_exception(&exception_object);
    let (stdout, stderr) = get_captured_stdout_stderr();

    Error {
      kind,
      message,
      traceback,
      stdout,
      stderr,
    }
  }

  pub fn is_assertion_error(&self) -> bool {
    self.kind == "AssertionError"
  }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TracebackFrame {
  pub line: usize,
  pub function: String,
  pub file: path::PathBuf,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Trackback {
  pub frames: Vec<TracebackFrame>,
}
impl Trackback {
  fn from_exception(exception: &PyObject) -> Option<Self> {
    // Handle the exception manually else we end in an infinite loop of exceptions
    let traceback_ptr = unsafe { ffi::PyException_GetTraceback(exception.as_ptr()) };
    let traceback_object = ptr::NonNull::new(traceback_ptr).map(PyObject)?;

    let mut traceback = Self { frames: Vec::new() };
    traceback.add_frame(&traceback_object);
    Some(traceback)
  }

  fn add_frame(&mut self, frame: &PyObject) {
    let code = frame
      .get_attr_cstr(c"tb_frame")
      .unwrap()
      .get_attr_cstr(c"f_code")
      .unwrap();

    let line = frame.get_attr_cstr(c"tb_lineno").unwrap().as_usize();
    let function = code.get_attr_cstr(c"co_name").unwrap().to_string();
    let file = code.get_attr_cstr(c"co_filename").unwrap().to_string();

    self.frames.push(TracebackFrame {
      line,
      function,
      file: path::PathBuf::from(file),
    });

    let next_frame = frame.get_attr_cstr(c"tb_next").unwrap();
    if !next_frame.is_none() {
      self.add_frame(&next_frame);
    }
  }
}

/// Compile the given file into a [`PyObject`] containing the bytecode
pub fn compile_file(file: &path::Path) -> Result<PyObject, Error> {
  let file = &file.canonicalize().expect("file to exist");

  add_all_parent_modules_to_path(file);
  capture_stdout_stderr();

  let file_name: CString = filename_to_cstring(file);
  let source: CString = read_file_to_c_str(file).unwrap();

  unsafe {
    PyObject::new(ffi::Py_CompileString(
      source.as_ptr(),
      file_name.as_ptr(),
      ffi::Py_file_input,
    ))
  }
}

/// Execute the given and returns the [`PyObject`] representing the executed module, or an error which has arisen
pub fn execute_file(file: &path::Path) -> Result<PyObject, Error> {
  let file_name: CString = filename_to_cstring(file);
  let module_name: CString = calculate_module_name(file);
  let bytecode_object = compile_file(file)?;

  unsafe {
    // Execute the compiled bytecode module
    PyObject::new(ffi::PyImport_ExecCodeModuleEx(
      module_name.as_ptr(),
      bytecode_object.as_ptr(),
      file_name.as_ptr(),
    ))
  }
}

/// Execute a string of python code as a module
fn execute_string(string: &CStr) -> Result<PyObject, Error> {
  let filename = c"<xc internals>";

  unsafe {
    // Compile the sourcecode into Bytecode
    let bytecode_object = PyObject::new(ffi::Py_CompileString(
      string.as_ptr(),
      filename.as_ptr(),
      ffi::Py_file_input,
    ))?;

    // Execute the compiled bytecode module
    PyObject::new(ffi::PyImport_ExecCodeModuleEx(
      filename.as_ptr(),
      bytecode_object.as_ptr(),
      filename.as_ptr(),
    ))
  }
}

/// Adds the given path to Python's module resolution path variable.
///
/// Most commonly used to add the current folder to the module search path.
/// Assumes Python Interpreter is currently active.
fn add_to_sys_modules_path(path: &CStr) {
  unsafe {
    let sys_module = ffi::PyImport_ImportModule(c"sys".as_ptr());
    let path_list = ffi::PyObject_GetAttrString(sys_module, c"path".as_ptr());

    ffi::PyList_Insert(path_list, 0, ffi::PyUnicode_FromString(path.as_ptr()))
  };
}

/// Adds the given path and any parent paths which are Python files to
/// Python's module resolution path variable.
///
/// Used to ensure that a test module can be found no matter how it is specified.
fn add_all_parent_modules_to_path(file: &path::Path) {
  if file.is_dir() {
    add_to_sys_modules_path(&path_to_cstring(file));
  }

  if is_python_module(file) {
    if let Some(parent) = file.parent() {
      add_all_parent_modules_to_path(parent);
    }
  }
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

/// Read the entire contents of a file into a [`CString`].
///
/// Returns an error if the given file doesn't exist.
fn read_file_to_c_str(path: &path::Path) -> io::Result<CString> {
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

/// Redirect stdout and stderr from Python into a string
///
/// Captured output can be fetched by [`get_captured_stdout_stderr`]
fn capture_stdout_stderr() {
  execute_string(
    c"
import io, sys
sys.stdout = io.StringIO()
sys.stderr = io.StringIO()
    ",
  )
  .expect("code to run");
}

/// Get the captured stdout and stderr
fn get_captured_stdout_stderr() -> (String, String) {
  let module = execute_string(
    c"
import io, sys
stdout = sys.stdout.getvalue()
stderr = sys.stderr.getvalue()
    ",
  )
  .expect("code to run");

  let stdout = module.get_attr_cstr(c"stdout").expect("var to exist");
  let stderr = module.get_attr_cstr(c"stderr").expect("var to exist");

  (stdout.to_string(), stderr.to_string())
}

/// Get the path to the standard library
///
/// This is used to exclude the standard library from coverage. The `os` module is the
/// only module which has the `__file__` attribute.
fn get_standard_library_path() -> String {
  unsafe {
    let os_module = PyObject::new(ffi::PyImport_ImportModule(c"os".as_ptr())).unwrap();
    let os_module_file = os_module.get_attr_cstr(c"__file__").unwrap().to_string();

    os_module_file.strip_suffix("os.py").unwrap().to_owned()
  }
}
