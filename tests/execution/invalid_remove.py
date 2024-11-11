"""
Removes/ replaces functions/ classes so they don't exist when they are called at the test stage

- test_function: NON-TEST FAIL
- TestSuite.test_method: NON-TEST FAIL
- TestSuiteRemove.test_method: NON-TEST FAIL
- TestSuiteReplace.test_replaced_method: FAIL { "kind": "TypeError", "message": "'int' object is not callable"}
"""

import unittest


def test_function():
    assert True


del test_function


class TestSuite(unittest.TestCase):
    def test_method(self):
        assert True


del TestSuite


class TestSuiteRemove(unittest.TestCase):
    def __init__(self):
        del self.test_method

    def test_method(self):
        assert True


class TestSuiteReplace(unittest.TestCase):
    def __init__(self):
        setattr(self, "test_replaced_method", 5)

    def test_replaced_method(self):
        assert True
