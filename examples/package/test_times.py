"""Test importing from a sub module"""

import unittest

from ..times import parse_time


class TestParseStringToTime(unittest.TestCase):
    def test_invalid_time(self) -> None:
        self.assertEqual(parse_time("abc:de"), 0)
        self.assertEqual(parse_time("m5"), 0)
        self.assertEqual(parse_time("w2 m6"), 0)
