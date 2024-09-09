"""Method doesn't have self parameter so will raise error when called"""

import unittest


def add(a, b):
    return a + b


class TestAdd(unittest.TestCase):
    def test_add():
        assert add(1, 2) == 3
        assert add(1, 3) == 4
        assert add(1, 4) == 5
