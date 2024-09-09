"""These tests should fail"""


def test_regular_fail():
    assert (1 + 3) == 5


def test_other_error():
    variable = 5
    variable[4]
