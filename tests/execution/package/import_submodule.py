"""
Test importing from a sub module

- test_invalid_time: PASS
"""

from ..times import parse_time


def test_invalid_time():
    assert parse_time("abc:de") == 0
    assert parse_time("m5") == 0
    assert parse_time("w2 m6") == 0
