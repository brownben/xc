"""Expecting an error on function"""

import unittest


@unittest.expectedFailure
def test_will_fail():
    assert 4 == 5


@unittest.expectedFailure
def test_wont_fail():
    pass
