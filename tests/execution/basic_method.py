"""
Basic test in the class/method format

- TestStringMethods.test_upper: PASS
- TestStringMethods.test_isupper: PASS
- TestStringMethods.test_split: PASS
"""

import unittest


class TestStringMethods(unittest.TestCase):
    def test_upper(self):
        self.assertEqual("foo".upper(), "FOO")

    def test_isupper(self):
        self.assertTrue("FOO".isupper())
        self.assertFalse("Foo".isupper())

    def test_split(self):
        s = "hello world"
        self.assertEqual(s.split(), ["hello", "world"])
        # check that s.split fails when the separator is not a string
        with self.assertRaises(TypeError):
            s.split(2)

    # this shouldn't be run as doesn't start with test
    def other(self):
        assert False
