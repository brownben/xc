"""
Import decimal module.

The module causes problems if it is not first imported in the main interpreter.
Check that it is handled properly and can be used.

- test_import_first: PASS
- test_import_again: PASS
"""

from decimal import Decimal


def test_import_first():
    import decimal

    assert Decimal(0) == decimal.Decimal(0)


def test_import_again():
    import decimal

    assert Decimal(0) == decimal.Decimal(0)
