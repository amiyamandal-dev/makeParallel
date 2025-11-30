#!/usr/bin/env python3
"""
Simple test for task dependencies.
"""

import time
import makeparallel as mp

print("Testing dependencies...")

# Test 1: Basic dependency
print("\n[TEST 1] Basic dependency")

@mp.parallel_with_deps
def first():
    print("  Executing first task")
    time.sleep(0.2)
    return "result_from_first"

@mp.parallel_with_deps
def second(deps):
    print(f"  Executing second task with deps: {deps}")
    return f"processed_{deps[0]}"

h1 = first()
h2 = second(depends_on=[h1])

r1 = h1.get()
r2 = h2.get()

print(f"First: {r1}")
print(f"Second: {r2}")

assert r1 == "result_from_first"
assert "result_from_first" in r2
print("✓ PASSED")

# Test 2: Multiple dependencies
print("\n[TEST 2] Multiple dependencies")

@mp.parallel_with_deps
def task_a():
    print("  Task A")
    return "A"

@mp.parallel_with_deps
def task_b():
    print("  Task B")
    return "B"

@mp.parallel_with_deps
def task_c(deps):
    print(f"  Task C got: {deps}")
    return f"{deps[0]}+{deps[1]}"

ha = task_a()
hb = task_b()
hc = task_c(depends_on=[ha, hb])

ra = ha.get()
rb = hb.get()
rc = hc.get()

print(f"A: {ra}, B: {rb}, C: {rc}")
assert rc == "A+B"
print("✓ PASSED")

# Test 3: Dependency chain
print("\n[TEST 3] Dependency chain")

@mp.parallel_with_deps
def step1():
    return 1

@mp.parallel_with_deps
def step2(deps):
    return deps[0] + 1

@mp.parallel_with_deps
def step3(deps):
    return deps[0] + 1

h1 = step1()
h2 = step2(depends_on=[h1])
h3 = step3(depends_on=[h2])

result = h3.get()

print(f"Chain result: {result}")
assert result == 3
print("✓ PASSED")

print("\n✓ ALL DEPENDENCY TESTS PASSED")
