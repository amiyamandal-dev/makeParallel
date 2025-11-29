#!/usr/bin/env python3
"""
Test script for PyO3 decorators
"""

import time
from makeParallel import timer, log_calls, CallCounter, retry, memoize

print("=" * 60)
print("Testing PyO3 Decorators")
print("=" * 60)

# Test 1: Timer decorator
print("\n1. Testing @timer decorator:")
print("-" * 40)

@timer
def slow_function(n):
    """A function that takes some time"""
    time.sleep(0.1)
    return sum(range(n))

result = slow_function(1000)
print(f"Result: {result}\n")

# Test 2: Log calls decorator
print("\n2. Testing @log_calls decorator:")
print("-" * 40)

@log_calls
def calculate_sum(a, b, multiplier=1):
    """Calculate sum with optional multiplier"""
    return (a + b) * multiplier

result = calculate_sum(5, 3)
result = calculate_sum(10, 20, multiplier=2)
print()

# Test 3: CallCounter decorator (PyClass)
print("\n3. Testing CallCounter decorator (PyClass):")
print("-" * 40)

@CallCounter
def greet(name):
    """Greet someone"""
    return f"Hello, {name}!"

print(greet("Alice"))
print(greet("Bob"))
print(greet("Charlie"))
print(f"Total calls: {greet.call_count}")

# Reset the counter
greet.reset()
print("Counter reset!")
print(greet("Dave"))
print(f"Total calls after reset: {greet.call_count}\n")

# Test 4: Retry decorator with arguments
print("\n4. Testing @retry decorator:")
print("-" * 40)

attempt_count = 0

@retry(max_retries=3)
def flaky_function():
    """Function that fails twice then succeeds"""
    global attempt_count
    attempt_count += 1
    if attempt_count < 3:
        raise ValueError(f"Attempt {attempt_count} failed!")
    return "Success!"

try:
    result = flaky_function()
    print(f"Final result: {result}")
except Exception as e:
    print(f"Failed: {e}")

print()

# Test 5: Retry with actual failure
print("\n5. Testing @retry with permanent failure:")
print("-" * 40)

@retry(max_retries=2)
def always_fails():
    """Function that always fails"""
    raise RuntimeError("This always fails!")

try:
    result = always_fails()
except RuntimeError as e:
    print(f"Caught expected error: {e}\n")

# Test 6: Memoize decorator
print("\n6. Testing @memoize decorator:")
print("-" * 40)

@memoize
def expensive_calculation(n):
    """Simulate expensive calculation"""
    print(f"  Computing fibonacci({n})...")
    time.sleep(0.1)  # Simulate expensive operation
    if n <= 1:
        return n
    # Note: This is inefficient without the memoization
    return n * 2  # Simplified for demo

print("First call:")
result1 = expensive_calculation(10)
print(f"Result: {result1}")

print("\nSecond call (should use cache):")
result2 = expensive_calculation(10)
print(f"Result: {result2}")

print("\nThird call with different argument:")
result3 = expensive_calculation(20)
print(f"Result: {result3}")

print("\nFourth call (should use cache):")
result4 = expensive_calculation(10)
print(f"Result: {result4}\n")

# Test 7: Combining decorators
print("\n7. Testing combined decorators:")
print("-" * 40)

@timer
@log_calls
def combined_function(x, y):
    """Function with multiple decorators"""
    return x ** y

result = combined_function(2, 10)
print()

# Test 8: Decorator on class method
print("\n8. Testing decorator on class method:")
print("-" * 40)

class Calculator:
    @timer
    def multiply(self, a, b):
        time.sleep(0.05)
        return a * b

    @CallCounter
    def add(self, a, b):
        return a + b

calc = Calculator()
print(f"Multiply: {calc.multiply(5, 7)}")
print(f"Add: {calc.add(3, 4)}")
print(f"Add: {calc.add(10, 20)}")
print(f"Add call count: {calc.add.call_count}")

print("\n" + "=" * 60)
print("All tests completed successfully!")
print("=" * 60)
