#!/usr/bin/env python3
"""
Simple test for callbacks.
"""

import time
import makeparallel as mp

print("Testing callbacks...")

# Test 1: on_complete
print("\n[TEST 1] on_complete")
complete_results = []

@mp.parallel
def task1():
    time.sleep(0.2)
    return "done"

handle = task1()
handle.on_complete(lambda r: complete_results.append(r))
result = handle.get()
time.sleep(0.1)

print(f"Result: {result}")
print(f"Callback got: {complete_results}")
assert result == "done"
print("✓ PASSED")

# Test 2: on_progress
print("\n[TEST 2] on_progress")
progress_updates = []

@mp.parallel
def task2():
    for i in range(3):
        mp.report_progress((i+1)/3)
        time.sleep(0.1)
    return "finished"

handle = task2()
handle.on_progress(lambda p: progress_updates.append(p))
result = handle.get()
time.sleep(0.1)

print(f"Progress: {progress_updates}")
print(f"Result: {result}")
assert len(progress_updates) > 0
print("✓ PASSED")

# Test 3: on_error
print("\n[TEST 3] on_error")
errors = []

@mp.parallel
def task3():
    raise ValueError("test error")

handle = task3()
handle.on_error(lambda e: errors.append(str(e)))

try:
    handle.get()
except:
    pass

time.sleep(0.1)
print(f"Errors: {errors}")
assert len(errors) > 0
print("✓ PASSED")

print("\n✓ ALL CALLBACK TESTS PASSED")
