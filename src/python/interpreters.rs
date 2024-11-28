use pyo3_ffi::{self as ffi};
use std::{env, mem, ptr};
use widestring::WideCString;

use super::ActiveInterpreter;

/// Interface implemented by both [`MainInterpreter`] and [`SubInterpreter`]
///
/// Allows Python Code to be executed on both in a unified manner, whilst acquiring the GIL
pub trait Interpreter {
  fn get_interpreter_state(&self) -> *mut ffi::PyThreadState;
  fn set_interpreter_state(&mut self, state: *mut ffi::PyThreadState);

  fn with_gil<T: Send>(&mut self, action: impl Fn(&ActiveInterpreter) -> T) -> T {
    // Get the Global Interpreter Lock (GIL) for the interpreter
    unsafe { ffi::PyEval_RestoreThread(self.get_interpreter_state()) };

    // SAFETY: We have the GIL
    let result = action(unsafe { &ActiveInterpreter::new() });

    // Release the GIL, and save the interpreter state
    self.set_interpreter_state(unsafe { ffi::PyEval_SaveThread() });

    result
  }
}

/// Represents the main Python Interpreter
///
/// Initialise's the interpreter on creation, and stores the thread state so
/// it can be cleaned up correctly on drop.
///
/// SAFETY: It must exist for any operations with Python to work
pub struct MainInterpreter {
  main_thread_state: *mut ffi::PyThreadState,
  _virtual_enviroment_path: WideCString,
}
impl MainInterpreter {
  pub fn initialize() -> Self {
    let mut config: mem::MaybeUninit<ffi::PyConfig> = mem::MaybeUninit::uninit();
    let mut virtual_enviroment_path: WideCString = WideCString::new();

    unsafe {
      ffi::PyConfig_InitPythonConfig(ptr::from_mut(config.assume_init_mut()));

      if let Ok(virtual_enviroment) = env::var("VIRTUAL_ENV") {
        virtual_enviroment_path = WideCString::from_str(&virtual_enviroment).unwrap();
        config.assume_init_mut().home = virtual_enviroment_path.as_mut_ptr().cast();
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
impl Interpreter for MainInterpreter {
  fn get_interpreter_state(&self) -> *mut ffi::PyThreadState {
    self.main_thread_state
  }

  fn set_interpreter_state(&mut self, state: *mut ffi::PyThreadState) {
    self.main_thread_state = state;
  }
}
impl Drop for MainInterpreter {
  fn drop(&mut self) {
    unsafe { ffi::PyEval_RestoreThread(self.main_thread_state) };
    unsafe { ffi::Py_FinalizeEx() };
  }
}
// We can share a reference to the interpreter between threads, as any actions performed
// Get the GIL first
unsafe impl Sync for MainInterpreter {}

/// Represents a Python Subinterpreter (An interpreter for a specific thread)
pub struct SubInterpreter {
  interpreter_state: *mut ffi::PyThreadState,
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
  pub fn new(main: &MainInterpreter) -> Self {
    let mut interpreter_state = std::ptr::null_mut();

    unsafe {
      ffi::PyEval_RestoreThread(main.main_thread_state); // ensure the main GIL is held
      ffi::Py_NewInterpreterFromConfig(&mut interpreter_state, &Self::DEFAULT_CONFIG);
      ffi::PyEval_SaveThread(); // Releases the GIL of the new subinterpreter
                                // the main GIL is released during creation
    };

    Self { interpreter_state }
  }
}
impl Interpreter for SubInterpreter {
  fn get_interpreter_state(&self) -> *mut ffi::PyThreadState {
    self.interpreter_state
  }

  fn set_interpreter_state(&mut self, state: *mut ffi::PyThreadState) {
    self.interpreter_state = state;
  }
}
impl Drop for SubInterpreter {
  fn drop(&mut self) {
    unsafe { ffi::PyEval_RestoreThread(self.interpreter_state) };
    unsafe { ffi::Py_EndInterpreter(self.interpreter_state) };
  }
}
