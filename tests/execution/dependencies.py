"""
These tests test importing a dependency and using it in a test
From issue #1 (https://github.com/brownben/xc/issues/1)

- test_astroid: PASS
- test_yarl: PASS
"""

import astroid
from yarl import URL


def test_astroid():
    source_code = """
    def foo(bar: str) -> int:
        return len(bar)
    """
    tree = astroid.parse(source_code)
    assert tree


def test_yarl():
    foo = URL("http://foo.com/")
    assert foo / "bar"
