"""Skipping a test. Nothing should be run from this file"""

import unittest


@unittest.skip(reason="To test skipping")
def test_one():
    assert 4 == 5


class TestSuite(unittest.TestCase):
    @unittest.skip(reason="Skipping a method")
    def test(self):
        assert 4 == 5


@unittest.skip(reason="Skipping a class")
class TestSuiteTwo(unittest.TestCase):
    def test(self):
        assert 4 == 5

def skipping_by_raising_an_exception_test():
    raise unittest.SkipTest("Skipping a function by raising")
