#!/usr/bin/env python3
"""Minimal test for @parallel"""

from makeParallel import parallel
import time

print("Testing minimal parallel decorator...")

@parallel
def simple_add(a, b):
    print(f"  In thread: adding {a} + {b}")
    return a + b

print("1. Calling function...")
handle = simple_add(5, 3)

print("2. Is ready?", handle.is_ready())

print("3. Getting result...")
result = handle.get()

print(f"4. Result: {result}")
print("5. Is ready now?", handle.is_ready())

print("\nTest complete!")
