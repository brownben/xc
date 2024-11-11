"""
Test the ways to skip, or mark as expected failure a test using `pytest`

- test_skip_mark: SKIP
- test_skip_throw: SKIP
- test_skipif_true: SKIP
- test_skipif_false: FAIL
- test_expected_failure: PASS
- test_should_fail_on_condition: PASS
- test_should_fail_on_condition_ignored: FAIL
"""

import pytest


@pytest.mark.skip(reason="to test skipping")
def test_skip_mark():
    assert False


def test_skip_throw():
    pytest.skip("it will throw Skipped")


@pytest.mark.skipIf(True, reason="to test skipping")
def test_skipif_true():
    assert False


@pytest.mark.skipIf(False, reason="to test (not) skipping")
def test_skipif_false():
    assert False


@pytest.mark.xfail
def test_expected_failure():
    assert False


@pytest.mark.xfail(True)
def test_should_fail_on_condition():
    assert False


@pytest.mark.xfail(False)
def test_should_fail_on_condition_ignored():
    assert False
