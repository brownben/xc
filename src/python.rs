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
        config.assume_init_mut().prefix = virtual_enviroment_path.as_mut_ptr().cast();
      }

      ffi::Py_InitializeFromConfig(ptr::from_mut(config.assume_init_mut()));
    }

    // The decimal module crashes the interpreter if it is initialised multiple times
    // If not initialised in the base interpreter, if a subinterpreter imports it it will crash
    // TODO: when decimal works properly remove this hack
    PyObject::import_module(c"decimal").expect("decimal module to exist");

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
unsafe impl Sync for Interpreter {}

/// Represents a Python Subinterpreter (An interpreter for a specific thread)
pub struct SubInterpreter {
  interpreter_state: *mut ffi::PyThreadState,
  coverage_trace_object: Option<PyObject>,
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
  pub fn new(main: &Interpreter) -> Self {
    let mut interpreter_state = std::ptr::null_mut();

    unsafe {
      ffi::PyEval_RestoreThread(main.main_thread_state); // ensure the main GIL is held
      ffi::Py_NewInterpreterFromConfig(&mut interpreter_state, &Self::DEFAULT_CONFIG);
      ffi::PyEval_SaveThread(); // Releases the GIL of the new subinterpreter
                                // the main GIL is released during creation
    };

    Self {
      interpreter_state,
      coverage_trace_object: None,
    }
  }

  pub fn enable_coverage(&mut self) {
    unsafe { ffi::PyEval_RestoreThread(self.interpreter_state) };

    self.coverage_trace_object = Some(coverage::enable_collection());

    self.interpreter_state = unsafe { ffi::PyEval_SaveThread() };
  }

  pub fn get_coverage(&mut self) -> Option<coverage::Lines> {
    unsafe { ffi::PyEval_RestoreThread(self.interpreter_state) };

    let result = mem::take(&mut self.coverage_trace_object)
      .as_ref()
      .map(coverage::get_executed_lines);

    self.interpreter_state = unsafe { ffi::PyEval_SaveThread() };

    result
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

  pub fn import_module(module_name: &CStr) -> Result<PyObject, Error> {
    let module = unsafe { ffi::PyImport_ImportModule(module_name.as_ptr()) };
    Self::new(module)
  }

  pub fn as_ptr(&self) -> *mut ffi::PyObject {
    self.0.as_ptr()
  }
  fn as_usize(&self) -> usize {
    debug_assert!(unsafe { ffi::PyLong_CheckExact(self.as_ptr()) == 1 });

    unsafe { ffi::PyLong_AsLongLong(self.as_ptr()) }
      .try_into()
      .unwrap()
  }

  /// Get the attribute of the object
  pub fn get_attr(&self, attribute_name: &str) -> Result<PyObject, Error> {
    let attribute_name = CString::new(attribute_name).unwrap();
    self.get_attr_cstr(&attribute_name)
  }
  /// Get the attribute of the object
  pub fn get_attr_cstr(&self, attribute_name: &CStr) -> Result<PyObject, Error> {
    let attribute_value =
      unsafe { ffi::PyObject_GetAttrString(self.as_ptr(), attribute_name.as_ptr()) };

    Self::new(attribute_value)
  }

  /// Set the attribute of the object
  pub fn set_attr_cstr(&self, attribute_name: &CStr, value: &PyObject) {
    unsafe { ffi::PyObject_SetAttrString(self.as_ptr(), attribute_name.as_ptr(), value.as_ptr()) };
  }

  /// Check if the object has an attribute
  pub fn has_attr(&self, attr: &CStr) -> bool {
    unsafe { ffi::PyObject_HasAttrString(self.as_ptr(), attr.as_ptr()) == 1 }
  }

  /// Check's if given attribute exists, and if it is set to a truthy value
  pub fn has_truthy_attr(&self, attr: &CStr) -> bool {
    let has_attr = self.has_attr(attr);

    if !has_attr {
      return false;
    }

    self.get_attr_cstr(attr).unwrap().is_truthy()
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
    debug_assert!(unsafe { ffi::PyIter_Check(iterator) == 1 });

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
  pub fn into_iter(self) -> impl Iterator<Item = PyObject> {
    unsafe { Self::new(ffi::PyObject_GetIter(self.as_ptr())) }
      .unwrap()
      .iter()
  }

  /// Is the object a code object?
  pub fn is_code_object(&self) -> bool {
    unsafe { ffi::PyCode_Check(self.as_ptr()) == 1 }
  }

  /// Assume is a tuple, and get the size of the tuple
  pub fn tuple_size(&self) -> isize {
    debug_assert!(unsafe { ffi::PyTuple_CheckExact(self.as_ptr()) == 1 });
    unsafe { ffi::PyTuple_Size(self.as_ptr()) }
  }

  /// Assume is a tuple, and get the item at the given index
  pub fn get_tuple_item(&self, index: isize) -> PyObject {
    debug_assert!(unsafe { ffi::PyTuple_Check(self.as_ptr()) == 1 });
    let result = unsafe { ffi::PyTuple_GetItem(self.as_ptr(), index) };
    let pointer = unsafe { ptr::NonNull::new_unchecked(result) };

    PyObject(pointer)
  }

  /// Assume is a dict, and get the item with the given key
  pub fn get_dict_item(&self, key: &CStr) -> Option<PyObject> {
    debug_assert!(unsafe { ffi::PyDict_CheckExact(self.as_ptr()) == 1 });
    let result = unsafe { ffi::PyDict_GetItemString(self.as_ptr(), key.as_ptr()) };
    if result.is_null() {
      None
    } else {
      Some(PyObject::new(result).unwrap())
    }
  }

  /// Assume is a Long, and get the value
  pub fn get_long(self) -> i32 {
    debug_assert!(unsafe { ffi::PyLong_CheckExact(self.as_ptr()) == 1 });

    #[allow(
      clippy::useless_conversion,
      reason = "`clong` is i32 on windows, i64 on unix"
    )]
    unsafe { ffi::PyLong_AsLong(self.as_ptr()) }
      .try_into()
      .unwrap()
  }

  /// Is the object truthy?
  pub fn is_truthy(&self) -> bool {
    unsafe { ffi::PyObject_IsTrue(self.as_ptr()) == 1 }
  }

  /// Clone the pointer without incrementing the reference count
  pub fn local_clone(&self) -> Self {
    Self(self.0)
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
  pub fn is_skip_exception(&self) -> bool {
    self.kind.starts_with("Skip")
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
pub(crate) fn execute_string(string: &CStr) -> Result<PyObject, Error> {
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
  let sys = PyObject::import_module(c"sys").unwrap();
  let path_list = sys.get_attr_cstr(c"path").unwrap();

  unsafe {
    let path_string = ffi::PyUnicode_FromString(path.as_ptr());
    ffi::PyList_Insert(path_list.as_ptr(), 0, path_string);
  }
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
  let sys = PyObject::import_module(c"sys").unwrap();
  let io = PyObject::import_module(c"io").unwrap();

  let string_io = io.get_attr_cstr(c"StringIO").unwrap();
  let stdout_io = string_io.local_clone().call().unwrap();
  let stderr_io = string_io.call().unwrap();

  sys.set_attr_cstr(c"stdout", &stdout_io);
  sys.set_attr_cstr(c"stderr", &stderr_io);
}

/// Get the captured stdout and stderr
fn get_captured_stdout_stderr() -> (String, String) {
  let sys = PyObject::import_module(c"sys").unwrap();

  let stdout = sys.get_attr_cstr(c"stdout").unwrap();
  let stderr = sys.get_attr_cstr(c"stderr").unwrap();

  let stdout_value = stdout.get_attr_cstr(c"getvalue").unwrap().call().unwrap();
  let stderr_value = stderr.get_attr_cstr(c"getvalue").unwrap().call().unwrap();

  (stdout_value.to_string(), stderr_value.to_string())
}
