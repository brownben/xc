"""
Expecting an error on function

- test_will_fail: PASS
- test_wont_fail: EXPECTED FAILURE
- TestSuite.method_will_fail_test: PASS
- TestSuite.method_wont_fail_test: EXPECTED FAILURE
"""

import unittest


@unittest.expectedFailure
def test_will_fail():
    assert 4 == 5


@unittest.expectedFailure
def test_wont_fail():
    pass


class TestSuite(unittest.TestCase):
    @unittest.expectedFailure
    def method_will_fail_test(self):
        assert False

    @unittest.expectedFailure
    def method_wont_fail_test(self):
        assert True
