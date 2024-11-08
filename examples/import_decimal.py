"""
Import decimal module.

Causes problems if the module is not first imported in the main interpreter.
"""

from decimal import Decimal


def test_import_first():
    import decimal

    assert Decimal(0) == decimal.Decimal(0)


def test_import_again():
    import decimal

    assert Decimal(0) == decimal.Decimal(0)
