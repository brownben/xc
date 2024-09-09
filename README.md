# xc 🏃‍♂️‍➡️

A parallel test runner for Python using subinterpreters, written in Rust.

- ⚡️ Run all your tests in parallel
- 🔬 Each test is isolated
- 🤝 Works with Python 3.13
- 🔎 Automatic test discovery
- 🦀 Written in Rust

`xc` aims to be a fast parallel test runner for Python. It statically finds tests across all your Python files, before running each test in its own thread. Each test is executed in its own subinterpreter, which means it is independent of all other tests. Subinterpreters allow multiple Python interpreters to be in the same process, rather than having to start a new process for each test.

However, subinterpreters are only available in Python 3.12+ and many external modules (such as `pydantic`) don't support them yet. Some standard library modules (like `decimal`) only work in Python 3.13.

## Usage

To install `xc`:

```sh
cargo install --git https://github.com/brownben/xc.git
```

To run tests:

```sh
xc                     # runs any tests found in the current directory
xc ./specific/folder   # runs tests found in specified folder
xc ./specific/file.py  # runs tests found in a specific file
xc ./a.py ./b.py       # multiple paths can be specified
```

Tests can be in `unitest` format:

```python
import unittest

class TestStringMethods(unittest.TestCase):
    def test_upper(self):
        self.assertEqual("foo".upper(), "FOO")
```

Or can be in a `pytest` style:

```python
def test_add():
    assert (1 + 2) == 3
```

## License

This repository is licensed under the [Apache-2.0 license](./LICENSE)
