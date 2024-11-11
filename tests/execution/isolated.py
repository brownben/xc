"""
Checks that the tests are run in isolation and don't affect each other

- test_counter_one: PASS
- test_counter_two: PASS
- test_counter_together: FAIL
- test_collection_one: PASS
- test_collection_two: PASS
- test_collection_together: FAIL
"""

counter = 0
collection = []


def test_counter_one():
    global counter

    assert counter == 0
    counter += 1
    assert counter == 1


def test_counter_two():
    global counter

    assert counter == 0
    counter += 1
    assert counter == 1


def test_counter_together():
    test_counter_one()
    test_counter_two()


def test_collection_one():
    global collection
    print(collection)

    assert collection == []
    collection.append(1)
    collection.append(2)

    assert collection[1] == 2
    assert len(collection) == 2


def test_collection_two():
    global collection
    print(collection)

    collection.extend([3, 4, 5])
    assert collection[1] == 4

    collection.append(6)
    assert len(collection) == 4


def test_collection_together():
    test_collection_one()
    test_collection_two()
