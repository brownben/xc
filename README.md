# xc 🏃‍♂️‍➡️

A parallel test runner for Python using subinterpreters, written in Rust.

- ⚡️ Run all your tests in parallel
- 🔬 Each test is isolated
- 📔 Integrated coverage statistics
- 🤝 Works with Python 3.13
- 🔎 Automatic test discovery
- 🦀 Written in Rust

`xc` aims to be a fast parallel test runner for Python. It statically finds tests across all your Python files, before running each test in its own thread. Each test is executed in its own subinterpreter, which means it is independent of all other tests. Subinterpreters with separate GILs allow multiple Python interpreters to be in the same process, rather than having to start a new process for each test.

However, subinterpreters with separate GILs are only available in Python 3.12+ and many external modules (such as `pydantic`) as well as some standard library modules (such as `decimal`) don't support them yet.

## Usage

To install `xc`:

```sh
cargo install --git https://github.com/brownben/xc.git
```

As it is built from source, it will build against the currently active Python version. If you want it built for a specific version, make sure to build it in the virtual enviroment for that version.

To run tests:

```sh
xc                     # runs any tests found in the current directory
xc ./specific/folder   # runs tests found in specified folder
xc ./specific/file.py  # runs tests found in a specific file
xc ./a.py ./b.py       # multiple paths can be specified
```

Tests can be in `unittest` format:

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

Add the `--coverage` flag to see coverage statistics:

```
╭─ Coverage
│  File                                                    Lines    Missed  Coverage
├─ .\examples\test_times.py                                   28         0    100.0%
├─ .\examples\simple_function.py                               7         0    100.0%
├─ .\examples\skip_test.py                                    13         3     76.9%
├─ .\examples\invalid_method.py                                9         4     55.6%
├─ .\examples\times.py                                        17         0    100.0%
╰──
```

## License

This repository is licensed under the [Apache-2.0 license](./LICENSE)
