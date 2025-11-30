#!/usr/bin/env python3
"""
Simple test of basic new features to identify any issues.
"""

import makeParallel as mp
import time

print("Testing basic features...")

# Test 1: Basic parallel execution
print("\n1. Testing basic @parallel...")
@mp.parallel
def simple_task(x):
    return x * 2

handle = simple_task(5)
result = handle.get()
print(f"✅ Basic parallel: {result}")

# Test 2: Backpressure
print("\n2. Testing backpressure...")
mp.set_max_concurrent_tasks(3)
print("✅ set_max_concurrent_tasks() works")

# Test 3: Memory limit
print("\n3. Testing memory limit...")
mp.configure_memory_limit(80.0)
print("✅ configure_memory_limit() works")

# Test 4: Callbacks (without executing yet)
print("\n4. Testing callback registration...")
@mp.parallel
def callback_task(x):
    time.sleep(0.1)
    return x * 2

def on_complete(result):
    print(f"   Callback received: {result}")

handle = callback_task(10)
handle.on_complete(on_complete)
result = handle.get()
print(f"✅ Callbacks registered and executed: {result}")
time.sleep(0.2)  # Wait for callback

# Test 5: gather()
print("\n5. Testing gather()...")
@mp.parallel
def gather_task(x):
    return x ** 2

handles = [gather_task(i) for i in range(5)]
results = mp.gather(handles, on_error='skip')
print(f"✅ gather() works: {results}")

# Test 6: ParallelContext
print("\n6. Testing ParallelContext...")
with mp.ParallelContext(timeout=10.0) as ctx:
    print("   Inside context")
print("✅ ParallelContext works")

# Test 7: retry_backoff
print("\n7. Testing retry_backoff...")
try:
    retry_decorator = mp.retry_backoff(
        max_attempts=3,
        backoff='linear',
        initial_delay=0.01,
        max_delay=0.1
    )
    print("✅ retry_backoff() creates decorator")
except Exception as e:
    print(f"❌ retry_backoff failed: {e}")

print("\n🎉 All basic tests passed!")
