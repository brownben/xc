"""
Raises an error at the module level, so no tests can be run

- test: NON-TEST FAIL
- TestSuite.test: NON-TEST FAIL
"""

import unittest

x = 5 + ""


def add(a, b):
    return a + b


def test():
    assert add(1, 2) == 3
    assert add(1, 3) == 4
    assert add(1, 4) == 5


class TestSuite(unittest.TestCase):
    def test(self):
        assert add(1, 2) == 3
        assert add(1, 3) == 4
        assert add(1, 4) == 5
