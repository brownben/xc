use pyo3_ffi::{self as ffi};
use std::{ffi::CStr, fmt, marker::PhantomData, ops::Deref, ptr::NonNull};

/// Represents a Python object
pub struct PyObject(NonNull<ffi::PyObject>);
impl PyObject {
  /// Gets the underlying pointer for the object
  pub fn as_ptr(&self) -> *mut ffi::PyObject {
    self.0.as_ptr()
  }

  /// Get a `PyObject` from a raw Python Object pointer
  pub(crate) fn from_ptr(ptr: *mut ffi::PyObject) -> Option<Self> {
    Some(Self(NonNull::new(ptr)?))
  }
  /// Get a `PyObject` from a raw Python Object pointer
  ///
  /// SAFETY: Assumes that the pointer is non-null
  pub(crate) unsafe fn from_ptr_unchecked(ptr: *mut ffi::PyObject) -> Self {
    Self(unsafe { NonNull::new_unchecked(ptr) })
  }
  /// Get a `PyObject` from a raw Python Object pointer, or if NULL, get a `PyError`
  pub(crate) fn from_ptr_or_error(ptr: *mut ffi::PyObject) -> Result<Self, PyError> {
    if let Some(object) = Self::from_ptr(ptr) {
      Ok(object)
    } else {
      Err(PyError::get())
    }
  }

  /// Does the attribute exist for the object?
  pub fn has_attr(&self, attribute: &PyObject) -> bool {
    unsafe { ffi::PyObject_HasAttr(self.as_ptr(), attribute.as_ptr()) == 1 }
  }
  /// Get the attribute of an object
  pub fn get_attr(&self, attribute: &PyObject) -> Result<PyObject, PyError> {
    let result = unsafe { ffi::PyObject_GetAttr(self.as_ptr(), attribute.as_ptr()) };

    Self::from_ptr_or_error(result)
  }
  /// Get the attribute of an object
  pub fn get_attr_cstr(&self, attribute: &CStr) -> Result<PyObject, PyError> {
    let result = unsafe { ffi::PyObject_GetAttrString(self.as_ptr(), attribute.as_ptr()) };

    Self::from_ptr_or_error(result)
  }
  /// Get the attribute of an object
  ///
  /// SAFETY: Assumes that the attribute exists
  pub unsafe fn get_attr_unchecked(&self, attribute: &PyObject) -> PyObject {
    let result = unsafe { ffi::PyObject_GetAttr(self.as_ptr(), attribute.as_ptr()) };

    unsafe { Self::from_ptr_unchecked(result) }
  }
  /// Set the attribute of an object
  #[expect(clippy::needless_pass_by_value, reason = "we want to take ownership")]
  pub fn set_attr(&self, attribute: &PyObject, value: PyObject) -> Result<(), PyError> {
    let result =
      unsafe { ffi::PyObject_SetAttr(self.as_ptr(), attribute.as_ptr(), value.as_ptr()) };

    if result == 0 {
      Ok(())
    } else {
      Err(PyError::get())
    }
  }

  /// Calls the given object with no parameters
  pub fn call(&self) -> Result<PyObject, PyError> {
    // No debug assert against being callable, as would crash if the test is not a function

    let ptr = unsafe { ffi::PyObject_CallNoArgs(self.as_ptr()) };
    Self::from_ptr(ptr).ok_or_else(PyError::get)
  }
  /// Calls the given object with no parameters
  ///
  /// SAFETY: Assumes that the object is callable and the function succeeds
  pub unsafe fn call_unchecked(&self) -> PyObject {
    debug_assert!(self.is_callable());

    let ptr = unsafe { ffi::PyObject_CallNoArgs(self.as_ptr()) };
    unsafe { Self::from_ptr_unchecked(ptr) }
  }

  /// Convert the object to an iterator
  ///
  /// SAFETY: Assumes that the object is an iterator
  #[expect(
    clippy::wrong_self_convention,
    reason = "works better with borrowed objects"
  )]
  pub fn into_iter(&self) -> PyIter {
    let iterator_ptr = unsafe { ffi::PyObject_GetIter(self.as_ptr()) };
    debug_assert!(!iterator_ptr.is_null());
    PyIter(unsafe { Self::from_ptr_unchecked(iterator_ptr) })
  }

  /// The name of the Type of the `PyObject`
  pub fn type_name(&self) -> String {
    let object_type = unsafe { ffi::Py_TYPE(self.as_ptr()) };
    let type_name = unsafe { ffi::PyType_GetName(object_type) };

    unsafe { PyObject::from_ptr_unchecked(type_name) }.to_string()
  }

  /// Assume is a Long, and get the value
  pub fn as_long(&self) -> i32 {
    debug_assert!(self.is_number());

    #[allow(
      clippy::useless_conversion,
      reason = "clong is i32 on windows, i64 on unix"
    )]
    unsafe { ffi::PyLong_AsLong(self.as_ptr()) }
      .try_into()
      .unwrap()
  }

  /// Is the object truthy?
  ///
  /// The equivalent to `!!x`.
  pub fn is_truthy(&self) -> bool {
    unsafe { ffi::PyObject_IsTrue(self.as_ptr()) == 1 }
  }

  /// Is the object callable?
  pub fn is_callable(&self) -> bool {
    unsafe { ffi::PyCallable_Check(self.as_ptr()) == 1 }
  }
  /// Is the object a code object?
  pub fn is_code_object(&self) -> bool {
    unsafe { ffi::PyCode_Check(self.as_ptr()) == 1 }
  }
  /// Is the current object a dict?
  pub fn is_dict(&self) -> bool {
    unsafe { ffi::PyDict_Check(self.as_ptr()) == 1 }
  }
  /// Is the object an iterator?
  pub fn is_iter(&self) -> bool {
    unsafe { ffi::PyIter_Check(self.as_ptr()) == 1 }
  }
  /// Is the object a module?
  pub fn is_module(&self) -> bool {
    unsafe { ffi::PyModule_Check(self.as_ptr()) == 1 }
  }
  /// Is the object `None`?
  pub fn is_none(&self) -> bool {
    unsafe { ffi::Py_IsNone(self.as_ptr()) == 1 }
  }
  /// Is the object a number?
  pub fn is_number(&self) -> bool {
    unsafe { ffi::PyNumber_Check(self.as_ptr()) == 1 }
  }
  /// Is the current object a tuple?
  pub fn is_tuple(&self) -> bool {
    unsafe { ffi::PyTuple_Check(self.as_ptr()) == 1 }
  }
}
impl Clone for PyObject {
  fn clone(&self) -> Self {
    unsafe { ffi::Py_IncRef(self.as_ptr()) };
    Self(self.0)
  }
}
impl Drop for PyObject {
  fn drop(&mut self) {
    unsafe { ffi::Py_DecRef(self.as_ptr()) };
  }
}
impl fmt::Display for PyObject {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let string_object = unsafe { ffi::PyObject_Str(self.as_ptr()) };

    let mut size = 0;
    let pointer = unsafe { ffi::PyUnicode_AsUTF8AndSize(string_object, &mut size) };

    let Ok(length) = usize::try_from(size) else {
      // There was an error by python in creating a string
      return Ok(());
    };

    // SAFETY: Python gives us a valid UTF-8 string
    let slice = unsafe { std::slice::from_raw_parts(pointer.cast::<u8>(), length) };
    let str = unsafe { std::str::from_utf8_unchecked(slice) };

    f.write_str(str)
  }
}

/// A borrowed reference to a Python Object
#[repr(transparent)]
pub struct BorrowedPyObject<'a> {
  object: PyObject,
  _lifetime: PhantomData<&'a ()>,
}
impl BorrowedPyObject<'_> {
  fn new(object: PyObject) -> Self {
    Self {
      object,
      _lifetime: PhantomData,
    }
  }
}
impl Deref for BorrowedPyObject<'_> {
  type Target = PyObject;

  fn deref(&self) -> &Self::Target {
    &self.object
  }
}
impl Drop for BorrowedPyObject<'_> {
  // We don't drop anything or decref reference counts, as this is a borrowed reference
  fn drop(&mut self) {}
}

/// A Python Dictionary Object
#[repr(transparent)]
pub struct PyDict(PyObject);
impl PyDict {
  /// Converts a [`PyObject`] into a [`PyDict`]
  pub fn from_object(object: PyObject) -> Option<Self> {
    object.is_dict().then_some(Self(object))
  }
  /// Converts a [`PyObject`] into a [`PyDict`]
  ///
  /// SAFETY: the `PyObject` must be a dict. Can be checked with [`PyObject::is_dict`]
  #[expect(dead_code)]
  pub unsafe fn from_object_unchecked(object: PyObject) -> Self {
    Self(object)
  }

  /// Gets an item from a dictionary
  pub fn get_item(&self, key: &PyObject) -> Option<BorrowedPyObject> {
    debug_assert!(self.is_dict());

    let ptr = unsafe { ffi::PyDict_GetItem(self.as_ptr(), key.as_ptr()) };

    if ptr.is_null() {
      None
    } else {
      let py_object = unsafe { PyObject::from_ptr_unchecked(ptr) };
      Some(BorrowedPyObject::new(py_object))
    }
  }

  /// Get a list of the items in the dictionary
  pub fn items(&self) -> PyObject {
    debug_assert!(self.is_dict());

    let ptr = unsafe { ffi::PyDict_Items(self.as_ptr()) };
    unsafe { PyObject::from_ptr_unchecked(ptr) }
  }
}
impl Deref for PyDict {
  type Target = PyObject;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

/// A Python Error/ Exception Object
#[repr(transparent)]
pub struct PyError(PyObject);
impl PyError {
  /// Get the currently raised exception. Panics if exception has not been raised.
  ///
  /// SAFETY: Assumes that the GIL is held
  pub fn get() -> Self {
    let ptr = unsafe { ffi::PyErr_GetRaisedException() };

    if let Some(ptr) = PyObject::from_ptr(ptr) {
      Self(ptr)
    } else {
      panic!("No exception has been raised");
    }
  }
  /// Clears an exception if one is set
  ///
  /// SAFETY: Assumes that the GIL is held
  pub fn clear() {
    unsafe { ffi::PyErr_Clear() };
  }

  /// Gets the Traceback object for this error
  pub fn get_traceback(&self) -> Option<PyObject> {
    let traceback_ptr = unsafe { ffi::PyException_GetTraceback(self.as_ptr()) };

    PyObject::from_ptr(traceback_ptr)
  }
}
impl Deref for PyError {
  type Target = PyObject;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}
impl fmt::Debug for PyError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_tuple("PyError").field(&self.0.as_ptr()).finish()
  }
}

/// A Python Iterator Object
#[repr(transparent)]
pub struct PyIter(PyObject);
impl PyIter {
  /// Converts a [`PyObject`] into a [`PyIter`]
  pub fn from_object(object: PyObject) -> Option<Self> {
    object.is_iter().then_some(Self(object))
  }
  /// Converts a [`PyObject`] into a [`PyIter`]
  ///
  /// SAFETY: the `PyObject` must be an iterator. Can be checked with [`PyObject::is_iter`]
  pub unsafe fn from_object_unchecked(object: PyObject) -> Self {
    Self(object)
  }
}
impl Iterator for PyIter {
  type Item = PyObject;

  fn next(&mut self) -> Option<Self::Item> {
    debug_assert!(self.is_iter());

    let next = unsafe { ffi::PyIter_Next(self.as_ptr()) };

    if next.is_null() {
      None
    } else {
      Some(unsafe { PyObject::from_ptr_unchecked(next) })
    }
  }
}
impl Deref for PyIter {
  type Target = PyObject;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

/// A Python Tuple Object
#[repr(transparent)]
pub struct PyTuple(PyObject);
impl PyTuple {
  /// Converts a [`PyObject`] into a [`PyTuple`]
  pub fn from_object(object: PyObject) -> Option<Self> {
    object.is_tuple().then_some(Self(object))
  }
  /// Converts a [`PyObject`] into a [`PyTuple`]
  ///
  /// SAFETY: the `PyObject` must be a tuple. Can be checked with [`PyObject::is_tuple`]
  pub unsafe fn from_object_unchecked(object: PyObject) -> Self {
    Self(object)
  }

  /// Get the size of a tuple
  pub fn size(&self) -> isize {
    debug_assert!(self.is_tuple());

    unsafe { ffi::PyTuple_Size(self.as_ptr()) }
  }

  /// Gets an item from a tuple
  ///
  /// SAFETY: index must be greater than 0, and less than the number of items in the tuple
  pub unsafe fn get_item_unchecked(&self, index: isize) -> BorrowedPyObject {
    debug_assert!(self.is_tuple());
    debug_assert!(index >= 0);
    debug_assert!(index < self.size());

    let ptr = unsafe { ffi::PyTuple_GET_ITEM(self.as_ptr(), index) };
    let py_object = unsafe { PyObject::from_ptr_unchecked(ptr) };
    BorrowedPyObject::new(py_object)
  }
  /// Gets an item from a tuple
  pub fn get_item(&self, index: isize) -> Result<BorrowedPyObject, PyError> {
    debug_assert!(self.is_tuple());
    debug_assert!(index >= 0);

    let ptr = unsafe { ffi::PyTuple_GET_ITEM(self.as_ptr(), index) };
    Ok(BorrowedPyObject::new(PyObject::from_ptr_or_error(ptr)?))
  }
}
impl Deref for PyTuple {
  type Target = PyObject;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}
