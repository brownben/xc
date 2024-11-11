"""
Skipping a test

- test_skip_function: SKIP
- TestSuite.test_skip_method: SKIP
- TestSuiteTwo.test_skip_class: SKIP
- skip_by_raising_an_exception_test: SKIP
- TestSuiteThree.test_skip_by_exception: SKIP
- test_skip_if_true: SKIP
- test_skip_if_false: FAIL
"""

import unittest


@unittest.skip(reason="To test skipping")
def test_skip_function():
    assert False


class TestSuite(unittest.TestCase):
    @unittest.skip(reason="Skipping a method")
    def test_skip_method(self):
        assert False


@unittest.skip(reason="Skipping a class")
class TestSuiteTwo(unittest.TestCase):
    def test_skip_class(self):
        assert False


def skip_by_raising_an_exception_test():
    raise unittest.SkipTest("Skipping a function by raising")


class TestSuiteThree(unittest.TestCase):
    def test_skip_by_exception(self):
        raise unittest.SkipTest("Skipping a function by raising")


@unittest.skipIf(True, reason="To test skipping")
def test_skip_if_true():
    assert False


@unittest.skipIf(False, reason="To test skipping")
def test_skip_if_false():
    assert False
