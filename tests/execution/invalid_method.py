"""
Code is invalid, so will raise error

- TestSuite.test_missing_self_parameter: FAIL { "kind": "TypeError", "message": "TestSuite.test_missing_self_parameter() takes 0 positional arguments but 1 was given" }
- BrokenClassTest.test_class_broken: NON-TEST FAIL
"""

import unittest


def add(a, b):
    return a + b


class TestSuite(unittest.TestCase):
    def test_missing_self_parameter():
        """Will raise an error when called"""

        assert add(1, 2) == 3
        assert add(1, 3) == 4
        assert add(1, 4) == 5


class BrokenClassTest(unittest.TestCase):
    def __init__(self):
        x = 5
        x[2]

    def test_class_broken():
        assert True
