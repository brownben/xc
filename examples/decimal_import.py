"""
On Python 3.12 decimal is not free threading safe, so can only be imported once. Thus running two tests which import decimal in separate subinterpreters fails.
"""

import decimal


def test_one():
    pass


def test_two():
    pass
