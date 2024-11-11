"""
Expecting an error on function

- test_will_fail: PASS
- test_wont_fail: EXPECTED FAILURE
"""

import unittest


@unittest.expectedFailure
def test_will_fail():
    assert 4 == 5


@unittest.expectedFailure
def test_wont_fail():
    pass
