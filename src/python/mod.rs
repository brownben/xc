mod interpreters;
pub mod objects;
mod operations;

use pyo3_ffi::{self as ffi};
use std::ffi::CStr;

pub use interpreters::{Interpreter, MainInterpreter, SubInterpreter};
pub use objects::{PyError, PyObject};
pub use operations::ActiveInterpreter;

pub fn version() -> String {
  let version = unsafe { CStr::from_ptr(ffi::Py_GetVersion()) };
  let platform = unsafe { CStr::from_ptr(ffi::Py_GetPlatform()) };

  let mut output = String::from(version.to_string_lossy());
  output.push(' ');
  output.push_str(&platform.to_string_lossy());
  output
}
