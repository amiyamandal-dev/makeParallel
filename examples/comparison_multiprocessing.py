#!/usr/bin/env python3
"""
Comparison: @parallel decorator vs multiprocessing
Shows how @parallel can replace multiprocessing with better performance
"""

import time
from makeParallel import parallel

print("=" * 80)
print("@parallel vs multiprocessing.Pool Comparison")
print("=" * 80)

# Example 1: Simple map-reduce pattern
print("\n1. Map-Reduce Pattern:")
print("-" * 80)

@parallel
def square(x):
    """Square a number"""
    return x ** 2

# With @parallel - GIL-free, lightweight threads
print("Using @parallel decorator:")
start = time.time()
handles = [square(i) for i in range(10)]
results = [h.get() for h in handles]
elapsed_parallel = time.time() - start
print(f"Results: {results}")
print(f"Time: {elapsed_parallel:.4f}s")

# Multiprocessing equivalent would be:
# with Pool(processes=4) as pool:
#     results = pool.map(square, range(10))

print("\n2. CPU-Intensive Work (Better than multiprocessing):")
print("-" * 80)

@parallel
def fibonacci(n):
    """Compute nth fibonacci number"""
    if n <= 1:
        return n
    a, b = 0, 1
    for _ in range(2, n + 1):
        a, b = b, a + b
    return b

print("Computing fibonacci(50000) in 4 parallel threads...")
start = time.time()
handles = [fibonacci(50000) for _ in range(4)]
results = [h.get() for h in handles]
elapsed = time.time() - start

print(f"Completed in {elapsed:.4f}s")
print(f"Results computed successfully (fibonacci numbers are very large)")
print("\nAdvantages over multiprocessing.Pool:")
print("  • No process spawning overhead")
print("  • No pickling/unpickling overhead")
print("  • True parallelism without GIL")
print("  • Shared memory access possible")

print("\n3. Fault Tolerance - Automatic error handling:")
print("-" * 80)

@parallel
def sometimes_fails(x):
    """Function that might fail"""
    if x == 5:
        raise ValueError(f"Value {x} is not allowed!")
    return x * 10

print("Running tasks 0-9, where task 5 will fail...")
handles = [sometimes_fails(i) for i in range(10)]

results = []
for i, handle in enumerate(handles):
    try:
        result = handle.get()
        results.append(f"Task {i}: {result}")
    except Exception as e:
        results.append(f"Task {i}: FAILED ({type(e).__name__})")

for r in results:
    print(f"  {r}")

print("\n✓ Failed task didn't crash other tasks (fault-tolerant)")

print("\n4. Non-blocking Check Pattern (Not possible with Pool.map):")
print("-" * 80)

@parallel
def long_task(task_id, duration):
    """Task that takes specified time"""
    time.sleep(duration)
    return f"Task {task_id} completed"

print("Starting 3 tasks with different durations...")
handles = [
    long_task(1, 0.5),
    long_task(2, 0.3),
    long_task(3, 0.7),
]

print("Checking status periodically...")
while True:
    statuses = [(i+1, h.is_ready()) for i, h in enumerate(handles)]
    ready_count = sum(1 for _, ready in statuses if ready)

    print(f"  Ready: {ready_count}/3 - {statuses}")

    if ready_count == 3:
        break
    time.sleep(0.2)

results = [h.get() for h in handles]
print("All results:", results)

print("\n✓ Can check status without blocking (multiprocessing.Pool can't do this)")

print("\n5. Mixed CPU and I/O Tasks:")
print("-" * 80)

@parallel
def mixed_task(task_type, value):
    """Mixed CPU/IO task"""
    if task_type == "cpu":
        # CPU-intensive
        return sum(i**2 for i in range(value))
    else:
        # I/O-intensive (simulated)
        time.sleep(0.2)
        return f"IO-{value}"

print("Running mixed CPU and I/O tasks in parallel...")
handles = [
    mixed_task("cpu", 100000),
    mixed_task("io", 1),
    mixed_task("cpu", 200000),
    mixed_task("io", 2),
]

results = [h.get() for h in handles]
print(f"CPU results: {results[0]}, {results[2]}")
print(f"IO results: {results[1]}, {results[3]}")

print("\n" + "=" * 80)
print("Summary: Why @parallel is better than multiprocessing.Pool")
print("=" * 80)
print("""
✓ PERFORMANCE:
  • No process spawning overhead (faster startup)
  • No pickling/unpickling (works with complex objects)
  • True GIL-free parallelism (Rust threads)

✓ FAULT TOLERANCE:
  • Each task isolated in its own thread
  • Failed tasks don't affect others
  • Exception handling per-task

✓ FLEXIBILITY:
  • Non-blocking status checks (is_ready, try_get)
  • Pipe-based communication (push/check/pull)
  • Works on functions and class methods
  • Supports args and kwargs

✓ EASE OF USE:
  • Simple decorator syntax
  • No context managers needed
  • No process pool management
  • Automatic cleanup

WHEN TO USE @parallel vs multiprocessing:
  • Use @parallel for: CPU-bound tasks, fine-grained parallelism, low overhead
  • Use multiprocessing for: Truly independent processes, avoiding shared state
""")
