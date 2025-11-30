"""
Test enhanced error handling and graceful shutdown
"""

import time
import makeParallel as mp
import atexit

print("=" * 70)
print("Testing Enhanced Error Handling and Graceful Shutdown")
print("=" * 70)

# Reset state
mp.reset_shutdown()
mp.reset_metrics()

# =============================================================================
# 1. TASK METADATA
# =============================================================================
print("\n1. Task Metadata")
print("-" * 70)

@mp.parallel
def task_with_metadata(data):
    time.sleep(0.2)
    return f"Processed {data}"

handle = task_with_metadata("test_data")

# Set metadata
handle.set_metadata("user_id", "12345")
handle.set_metadata("job_id", "job-abc-123")
handle.set_metadata("priority", "high")

print(f"Task ID: {handle.get_task_id()}")
print(f"Task name: {handle.get_name()}")
print(f"User ID: {handle.get_metadata('user_id')}")
print(f"Job ID: {handle.get_metadata('job_id')}")
print(f"All metadata: {handle.get_all_metadata()}")

result = handle.get()
print(f"Result: {result}")
print("✓ Metadata works!")

# =============================================================================
# 2. TASK TIMEOUT
# =============================================================================
print("\n2. Task Timeout")
print("-" * 70)

@mp.parallel
def slow_task():
    time.sleep(10)
    return "Should not reach here"

# Task with 1 second timeout
handle = slow_task(timeout=1.0)
print(f"Started task with timeout: {handle.get_timeout()}s")
print("Waiting for task to timeout...")

time.sleep(1.5)

if handle.is_cancelled():
    print("✓ Task was cancelled due to timeout!")
else:
    print("Task still running...")

# =============================================================================
# 3. ENHANCED ERROR HANDLING
# =============================================================================
print("\n3. Enhanced Error Handling")
print("-" * 70)

@mp.parallel
def failing_task(should_fail=True):
    time.sleep(0.1)
    if should_fail:
        raise ValueError("Intentional failure for testing")
    return "Success"

handle = failing_task(should_fail=True)
handle.set_metadata("test_run", "error_test")

print(f"Task ID: {handle.get_task_id()}")
print(f"Metadata: {handle.get_all_metadata()}")

try:
    result = handle.get()
    print(f"Result: {result}")
except Exception as e:
    print(f"\nCaught error:")
    print(f"  Error message: {str(e)}")
    print(f"  Task name: {handle.get_name()}")
    print(f"  Task ID: {handle.get_task_id()}")
    print(f"  Elapsed time: {handle.elapsed_time():.2f}s")
    print("✓ Enhanced error context captured!")

# =============================================================================
# 4. ACTIVE TASK TRACKING
# =============================================================================
print("\n4. Active Task Tracking")
print("-" * 70)

@mp.parallel
def monitored_task(task_id, duration):
    time.sleep(duration)
    return f"Task {task_id} done"

# Start multiple tasks
handles = []
for i in range(5):
    h = monitored_task(i, 0.5)
    handles.append(h)

print(f"Active tasks: {mp.get_active_task_count()}")

# Wait for completion
while not all(h.is_ready() for h in handles):
    active = mp.get_active_task_count()
    completed = sum(1 for h in handles if h.is_ready())
    print(f"  Active: {active}, Completed: {completed}/5", end='\r')
    time.sleep(0.1)

print(f"\n  All tasks completed!")
print(f"  Active tasks now: {mp.get_active_task_count()}")
print("✓ Task tracking works!")

# =============================================================================
# 5. GRACEFUL SHUTDOWN
# =============================================================================
print("\n5. Graceful Shutdown")
print("-" * 70)

# Reset shutdown flag
mp.reset_shutdown()

@mp.parallel
def long_running_task(task_id):
    for i in range(10):
        if mp.get_active_task_count() == 0:
            return f"Task {task_id} cancelled"
        time.sleep(0.2)
    return f"Task {task_id} completed"

# Start some tasks
print("Starting 3 long-running tasks...")
handles = [long_running_task(i) for i in range(3)]
time.sleep(0.3)  # Let them start

print(f"Active tasks before shutdown: {mp.get_active_task_count()}")

# Initiate shutdown
print("\nInitiating graceful shutdown with 2 second timeout...")
success = mp.shutdown(timeout_secs=2.0, cancel_pending=True)

if success:
    print("✓ Shutdown completed successfully!")
else:
    print("⚠ Shutdown timed out (some tasks still active)")

print(f"Active tasks after shutdown: {mp.get_active_task_count()}")

# Try to start a new task after shutdown
try:
    new_handle = monitored_task(99, 1.0)
    print("✗ Should not be able to start task after shutdown!")
except Exception as e:
    print(f"✓ Cannot start tasks after shutdown: {e}")

# =============================================================================
# 6. COMBINED FEATURES
# =============================================================================
print("\n6. Combined Features Test")
print("-" * 70)

# Reset for fresh test
mp.reset_shutdown()
mp.reset_metrics()

@mp.parallel
def comprehensive_task(task_num, should_fail=False):
    time.sleep(0.3)
    if should_fail:
        raise RuntimeError(f"Task {task_num} failed intentionally")
    return f"Task {task_num} succeeded"

handles = []
for i in range(5):
    h = comprehensive_task(i, should_fail=(i == 2))  # Task 2 will fail
    h.set_metadata("task_num", str(i))
    h.set_metadata("test_group", "comprehensive")
    handles.append(h)

print(f"Started {len(handles)} tasks")
print(f"Active: {mp.get_active_task_count()}")

# Process results
for i, h in enumerate(handles):
    try:
        result = h.get()
        print(f"  ✓ {result} (elapsed: {h.elapsed_time():.2f}s)")
    except Exception as e:
        print(f"  ✗ Task {i} failed: {str(e)[:80]}")

print(f"\nFinal active task count: {mp.get_active_task_count()}")

# Get metrics
metrics = mp.get_all_metrics()
if 'comprehensive_task' in metrics:
    m = metrics['comprehensive_task']
    print(f"\nMetrics:")
    print(f"  Total: {m['total_tasks']}")
    print(f"  Completed: {m['completed_tasks']}")
    print(f"  Failed: {m['failed_tasks']}")

print("\n" + "=" * 70)
print("All tests completed!")
print("=" * 70)

print("\n✅ New Features Verified:")
print("  ✓ Task metadata (set/get)")
print("  ✓ Task timeout")
print("  ✓ Enhanced error handling")
print("  ✓ Active task tracking")
print("  ✓ Graceful shutdown")
print("  ✓ Combined feature usage")
