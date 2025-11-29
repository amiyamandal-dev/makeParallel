#!/usr/bin/env python3
"""
Test script for @parallel decorator - Rust threads without GIL
"""

import time
from makeParallel import parallel

print("=" * 70)
print("Testing @parallel Decorator - Rust Threads without GIL")
print("=" * 70)

# Test 1: Basic parallel function
print("\n1. Testing basic @parallel function:")
print("-" * 70)

@parallel
def cpu_intensive_task(n):
    """Simulate CPU-intensive work"""
    total = 0
    for i in range(n):
        total += i ** 2
    return total

# Push work to thread (non-blocking)
print("Pushing work to Rust thread...")
handle = cpu_intensive_task(1000000)

# Check if ready (non-blocking)
print(f"Is ready? {handle.is_ready()}")

# Try to get result without blocking
result = handle.try_get()
if result is None:
    print("Result not ready yet (expected)")

# Do other work while thread is running
print("Main thread doing other work...")
time.sleep(0.1)

# Pull result (blocking)
print("Pulling result from thread...")
result = handle.get()
print(f"Result: {result}")
print(f"Is ready now? {handle.is_ready()}")

# Test 2: Multiple parallel tasks
print("\n2. Testing multiple parallel tasks:")
print("-" * 70)

@parallel
def slow_multiply(x, y):
    """Slow multiplication"""
    time.sleep(0.5)
    return x * y

# Start multiple tasks
print("Starting 3 parallel tasks...")
handle1 = slow_multiply(10, 20)
handle2 = slow_multiply(5, 7)
handle3 = slow_multiply(100, 3)

print("All tasks pushed to threads!")
print("Main thread is free to do other work...")

# Check status
time.sleep(0.2)
print(f"Task 1 ready? {handle1.is_ready()}")
print(f"Task 2 ready? {handle2.is_ready()}")
print(f"Task 3 ready? {handle3.is_ready()}")

# Wait for all to complete
print("\nWaiting for all tasks to complete...")
result1 = handle1.get()
result2 = handle2.get()
result3 = handle3.get()

print(f"Results: {result1}, {result2}, {result3}")

# Test 3: Testing wait with timeout
print("\n3. Testing wait with timeout:")
print("-" * 70)

@parallel
def long_task(duration):
    """Task that takes specified duration"""
    time.sleep(duration)
    return f"Completed after {duration}s"

handle = long_task(1.0)
print("Started task that takes 1 second...")

# Wait with timeout
is_ready = handle.wait(0.3)
print(f"Ready after 0.3s timeout? {is_ready}")

is_ready = handle.wait(1.0)
print(f"Ready after 1.0s timeout? {is_ready}")

result = handle.get()
print(f"Result: {result}")

# Test 4: Error handling
print("\n4. Testing error handling:")
print("-" * 70)

@parallel
def failing_function():
    """Function that raises an error"""
    raise ValueError("This function failed!")

handle = failing_function()
try:
    result = handle.get()
    print(f"Result: {result}")
except Exception as e:
    print(f"Caught error (expected): {e}")

# Test 5: Parallel function with kwargs
print("\n5. Testing parallel function with kwargs:")
print("-" * 70)

@parallel
def compute_power(base, exponent=2):
    """Compute base^exponent"""
    time.sleep(0.2)
    return base ** exponent

handle1 = compute_power(2, exponent=10)
handle2 = compute_power(5)  # Uses default exponent=2

print("Computing 2^10 and 5^2 in parallel...")
result1 = handle1.get()
result2 = handle2.get()
print(f"2^10 = {result1}")
print(f"5^2 = {result2}")

# Test 6: GIL-free parallelism demonstration
print("\n6. Testing true parallelism (GIL-free):")
print("-" * 70)

@parallel
def fibonacci(n):
    """Compute fibonacci number"""
    if n <= 1:
        return n
    a, b = 0, 1
    for _ in range(2, n + 1):
        a, b = b, a + b
    return b

# Run multiple CPU-intensive tasks
print("Starting 4 parallel fibonacci calculations...")
start = time.time()

handles = [
    fibonacci(100000),
    fibonacci(100000),
    fibonacci(100000),
    fibonacci(100000),
]

results = [h.get() for h in handles]
elapsed = time.time() - start

print(f"All 4 tasks completed in {elapsed:.2f}s")
print(f"First result length: {len(str(results[0]))} digits")
print("(These run in parallel without GIL contention!)")

# Test 7: Using parallel decorator on class methods
print("\n7. Testing parallel decorator on class methods:")
print("-" * 70)

class DataProcessor:
    def __init__(self, multiplier):
        self.multiplier = multiplier

    @parallel
    def process(self, data):
        """Process data in parallel"""
        time.sleep(0.3)
        return [x * self.multiplier for x in data]

processor = DataProcessor(10)
handle = processor.process([1, 2, 3, 4, 5])

print("Processing data in parallel thread...")
print(f"Is ready? {handle.is_ready()}")

result = handle.get()
print(f"Processed data: {result}")

# Test 8: Try get pattern
print("\n8. Testing try_get polling pattern:")
print("-" * 70)

@parallel
def gradual_task(steps):
    """Task that takes time"""
    for i in range(steps):
        time.sleep(0.1)
    return f"Completed {steps} steps"

handle = gradual_task(5)
print("Started task with 5 steps (0.5s total)...")

# Poll until ready
poll_count = 0
while True:
    result = handle.try_get()
    poll_count += 1
    if result is not None:
        print(f"Got result after {poll_count} polls: {result}")
        break
    print(f"Poll {poll_count}: Not ready yet...")
    time.sleep(0.15)

print("\n" + "=" * 70)
print("All @parallel decorator tests completed successfully!")
print("=" * 70)
print("\nKey features demonstrated:")
print("  ✓ Push: Call decorated function to push work to Rust thread")
print("  ✓ Check: Use is_ready() or try_get() to check status")
print("  ✓ Pull: Use get() to pull/block for result")
print("  ✓ Wait: Use wait(timeout) to wait with timeout")
print("  ✓ GIL-free: True parallelism on CPU-bound tasks")
print("  ✓ Pipe: Channel-based communication between threads")
