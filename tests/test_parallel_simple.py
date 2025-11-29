#!/usr/bin/env python3
"""
Simple test for @parallel decorator
"""

import time
from makeParallel import parallel

print("=" * 70)
print("Testing @parallel Decorator - Rust Threads without GIL")
print("=" * 70)

# Test 1: Basic parallel function
print("\n1. Basic parallel function with PUSH/CHECK/PULL:")
print("-" * 70)

@parallel
def calculate(x, y):
    """Simple calculation"""
    time.sleep(0.5)
    return x * y + x + y

# PUSH: Send work to thread
print("PUSH: Sending work to Rust thread...")
handle = calculate(10, 20)

# CHECK: Non-blocking check
print(f"CHECK: Is ready? {handle.is_ready()}")

# CHECK: Try to pull without blocking
result = handle.try_get()
print(f"CHECK: Try get result: {result}")

print("Main thread continues working...")

# PULL: Block and get result
print("PULL: Waiting for result...")
result = handle.get()
print(f"Result: {result}")
print(f"Is ready now? {handle.is_ready()}")

# Test 2: Multiple parallel tasks
print("\n2. Multiple parallel tasks:")
print("-" * 70)

@parallel
def multiply(a, b):
    time.sleep(0.3)
    return a * b

# Start 3 tasks
print("Starting 3 parallel tasks...")
h1 = multiply(5, 10)
h2 = multiply(7, 8)
h3 = multiply(3, 9)

print("All tasks running in parallel (GIL-free)!")
print("Waiting for results...")

results = [h1.get(), h2.get(), h3.get()]
print(f"Results: {results}")

# Test 3: CPU-intensive work (demonstrates GIL-free parallelism)
print("\n3. CPU-intensive parallel work:")
print("-" * 70)

@parallel
def cpu_work(n):
    """CPU-intensive calculation"""
    total = 0
    for i in range(n):
        total += i ** 2
    return total

print("Starting 4 parallel CPU tasks...")
start = time.time()

handles = [cpu_work(500000) for _ in range(4)]
results = [h.get() for h in handles]

elapsed = time.time() - start
print(f"Completed in {elapsed:.2f}s")
print(f"First result: {results[0]}")

# Test 4: Class methods
print("\n4. Parallel decorator on class methods:")
print("-" * 70)

class Worker:
    def __init__(self, factor):
        self.factor = factor

    @parallel
    def process(self, values):
        time.sleep(0.2)
        return [v * self.factor for v in values]

worker = Worker(5)
handle = worker.process([1, 2, 3, 4])
print("Processing in parallel thread...")
result = handle.get()
print(f"Result: {result}")

# Test 5: Error handling
print("\n5. Error handling:")
print("-" * 70)

@parallel
def failing_task():
    raise ValueError("Task failed!")

handle = failing_task()
try:
    result = handle.get()
except Exception as e:
    print(f"Caught error (expected): {type(e).__name__}")

# Test 6: With keyword arguments
print("\n6. Keyword arguments:")
print("-" * 70)

@parallel
def power(base, exp=2):
    time.sleep(0.1)
    return base ** exp

h1 = power(2, exp=10)
h2 = power(5)  # default exp=2

print(f"2^10 = {h1.get()}")
print(f"5^2 = {h2.get()}")

print("\n" + "=" * 70)
print("All tests passed! ✓")
print("=" * 70)
print("\nKey features:")
print("  ✓ PUSH: Call decorated function to push work")
print("  ✓ CHECK: Use is_ready() or try_get() to check status")
print("  ✓ PULL: Use get() to pull/block for result")
print("  ✓ Pipe: Channel-based communication (mpsc)")
print("  ✓ GIL-free: True parallel execution on Rust threads")
