"""
These tests should fail, and have the data written to stdout/stderr attatched to the error
"""

import sys


def test_stdout():
    print("hello world into stdout")
    assert False


def test_stderr():
    sys.stderr.write("hello world\ninto stderr")
    assert False
