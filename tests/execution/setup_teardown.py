"""
Tests the `setUp` and `tearDown` callbacks from unittest style tests

- SetUpTearDownTest.test: PASS
- SetUpFailTest.test: NON-TEST FAIL
- TearDownFailTest.test: NON-TEST FAIL
"""

import unittest


class Bomb:
    """Throws an error if not defused before it goes out of scope"""

    def __init__(self):
        self.active = True

    def defuse(self):
        self.active = False

    def __del__(self):
        if self.active:
            raise Exception("Didn't defuse the bomb")


class SetUpTearDownTest(unittest.TestCase):
    def setUp(self):
        self.x = 5
        self.bomb = Bomb()

    def test(self):
        self.assertEqual(self.x, 5)

    def tearDown(self) -> None:
        # checks that tearDown occurs, else bomb would throw error
        self.bomb.defuse()


class SetUpFailTest(unittest.TestCase):
    def setUp(self):
        raise Exception()

    def test(self):
        assert True


class TearDownFailTest(unittest.TestCase):
    def tearDown(self):
        raise Exception()

    def test(self):
        assert True
