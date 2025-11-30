#!/usr/bin/env python3
"""
Test script to verify the report_progress bug fix.
Tests both automatic task_id detection and explicit task_id usage.
"""

import time
import makeparallel as mp

# Test 1: Using report_progress inside a @parallel function (automatic task_id)
@mp.parallel
def long_task_with_progress(duration):
    """A task that reports its progress automatically."""
    steps = 10
    for i in range(steps):
        time.sleep(duration / steps)
        progress = (i + 1) / steps
        # Call report_progress without task_id - should use thread-local storage
        mp.report_progress(progress)
        print(f"  Progress: {progress * 100:.0f}%")
    return f"Completed after {duration}s"


# Test 2: Using report_progress with explicit task_id
@mp.parallel
def task_with_explicit_progress(duration, custom_id):
    """A task that reports progress with an explicit task_id."""
    steps = 5
    for i in range(steps):
        time.sleep(duration / steps)
        progress = (i + 1) / steps
        # Call report_progress with explicit task_id
        mp.report_progress(progress, task_id=custom_id)
        print(f"  Custom task {custom_id} progress: {progress * 100:.0f}%")
    return f"Custom task {custom_id} completed"


# Test 3: Get current task_id from within a parallel function
@mp.parallel
def task_that_checks_id():
    """A task that retrieves its own task_id."""
    task_id = mp.get_current_task_id()
    print(f"  My task_id is: {task_id}")

    # Report progress using the retrieved task_id
    for i in range(3):
        time.sleep(0.1)
        mp.report_progress((i + 1) / 3)

    return task_id


def main():
    print("=" * 60)
    print("Testing report_progress bug fix")
    print("=" * 60)

    # Test 1: Automatic task_id detection
    print("\n[Test 1] Using report_progress without task_id (automatic)")
    print("-" * 60)
    handle1 = long_task_with_progress(1.0)

    # Monitor progress
    while not handle1.is_ready():
        progress = handle1.get_progress()
        print(f"Main thread sees progress: {progress * 100:.0f}%")
        time.sleep(0.15)

    result1 = handle1.get()
    print(f"Result: {result1}")
    print(f"Final progress: {handle1.get_progress() * 100:.0f}%")

    # Test 2: Explicit task_id
    print("\n[Test 2] Using report_progress with explicit task_id")
    print("-" * 60)
    handle2 = task_with_explicit_progress(0.5, "my-custom-task")

    while not handle2.is_ready():
        time.sleep(0.15)

    result2 = handle2.get()
    print(f"Result: {result2}")

    # Test 3: Get current task_id
    print("\n[Test 3] Getting current task_id from within task")
    print("-" * 60)
    handle3 = task_that_checks_id()

    while not handle3.is_ready():
        progress = handle3.get_progress()
        print(f"Main thread sees progress: {progress * 100:.0f}%")
        time.sleep(0.15)

    result3 = handle3.get()
    print(f"Task reported its ID as: {result3}")
    print(f"Handle's task_id: {handle3.get_task_id()}")

    # Test 4: Error handling - calling report_progress outside parallel context
    print("\n[Test 4] Error handling - calling outside @parallel context")
    print("-" * 60)
    try:
        mp.report_progress(0.5)
        print("ERROR: Should have raised an exception!")
    except RuntimeError as e:
        print(f"✓ Correctly raised error: {e}")

    # Test 5: Multiple parallel tasks with progress
    print("\n[Test 5] Multiple parallel tasks with progress tracking")
    print("-" * 60)

    @mp.parallel
    def multi_task(task_num):
        steps = 5
        for i in range(steps):
            time.sleep(0.1)
            mp.report_progress((i + 1) / steps)
        return f"Task {task_num} done"

    handles = [multi_task(i) for i in range(3)]

    # Monitor all tasks
    all_done = False
    while not all_done:
        all_done = True
        for i, h in enumerate(handles):
            if not h.is_ready():
                all_done = False
                progress = h.get_progress()
                print(f"  Task {i}: {progress * 100:.0f}%", end="  ")
        if not all_done:
            print()
            time.sleep(0.15)

    results = [h.get() for h in handles]
    print(f"\nAll results: {results}")

    print("\n" + "=" * 60)
    print("All tests completed successfully! ✓")
    print("=" * 60)


if __name__ == "__main__":
    main()
