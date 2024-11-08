"""
Test the ways to skip a test using `pytest`

- Skip Mark
- SkipIf Mark
- Skip Exception
- Xfail Mark (Expected Failure)
"""

import pytest
import unittest


@pytest.mark.skip(reason="some reason")
def test_skip_mark():
    assert False


@unittest.expectedFailure
@pytest.mark.skipIf(False, reason="some reason")
def test_skipif_false():
    assert False


@pytest.mark.skipIf(True, reason="i don't know")
def test_skipif_true():
    assert False


def test_skip_throw():
    pytest.skip("it will throw Skipped")


@pytest.mark.xfail
def test_should_fail():
    assert False


@pytest.mark.xfail(True)
def test_should_fail_on_condition():
    assert False


@pytest.mark.xfail(False)
def test_should_fail_on_condition_ignored():
    assert True
