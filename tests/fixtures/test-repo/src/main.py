"""Test Python module for regression testing"""

class TestClass:
    """A test class for symbol indexing"""

    def __init__(self, name: str):
        self.name = name

    def test_method(self, value: int) -> int:
        """A test method"""
        return value * 2

def test_function(x: int, y: int) -> int:
    """A test function for symbol indexing"""
    return x + y

def another_function():
    """Another function"""
    pass
