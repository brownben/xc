"""
These tests should fail

- test_regular_fail: FAIL { "kind": "AssertionError" }
- test_other_error: FAIL { "kind": "TypeError" }
- TestSuite.test_method_fail: FAIL { "kind": "AssertionError" }
- TestSuite.test_method_other_error: FAIL { "kind": "TypeError" }
"""

import unittest


def test_regular_fail():
    """An assertion fails in this test"""
    assert (1 + 3) == 5


def test_other_error():
    """Some other error occurs in this test"""
    variable = 5
    variable[4]


class TestSuite(unittest.TestCase):
    def test_method_fail(self):
        assert (1 + 3) == 5

    def test_method_other_error(self):
        variable = 5
        variable[4]
