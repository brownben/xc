"""Each test takes about 6s to run, so can check it is run in parallel"""

import unittest


def fibonacci(n):
    if n <= 2:
        return 1
    else:
        return fibonacci(n - 1) + fibonacci(n - 2)


def test_one():
    fibonacci(40)


def test_two():
    fibonacci(40)


def test_three():
    fibonacci(40)
