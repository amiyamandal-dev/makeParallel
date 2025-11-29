"""
Test script for new makeParallel features:
1. Thread pool size configuration
2. Priority queues
3. Task cancellation improvements
4. Performance profiling tools
"""

import time
import makeParallel as mp

print("=" * 60)
print("Testing New makeParallel Features")
print("=" * 60)

# =============================================================================
# 1. THREAD POOL CONFIGURATION
# =============================================================================
print("\n1. Testing Thread Pool Configuration")
print("-" * 60)

# Check initial pool info
info = mp.get_thread_pool_info()
print(f"Initial thread pool info: {info}")

# Configure custom thread pool
mp.configure_thread_pool(num_threads=4)
info = mp.get_thread_pool_info()
print(f"After configuration: {info}")

# =============================================================================
# 2. PRIORITY QUEUES
# =============================================================================
print("\n2. Testing Priority Queues")
print("-" * 60)

@mp.parallel_priority
def priority_task(task_id, priority_level):
    """Task that prints with priority"""
    time.sleep(0.1)
    return f"Task {task_id} (priority={priority_level}) completed"

# Start the priority worker
mp.start_priority_worker()

# Submit tasks with different priorities (higher number = higher priority)
handles = []
priorities = [(1, 5), (2, 1), (3, 10), (4, 3), (5, 7)]

for task_id, priority in priorities:
    print(f"Submitting task {task_id} with priority {priority}")
    handle = priority_task(task_id, priority_level=priority, priority=priority)
    handles.append(handle)

# Get results (should execute in priority order: 3, 5, 1, 4, 2)
print("\nResults (in completion order):")
for handle in handles:
    result = handle.get()
    print(f"  {result}")

mp.stop_priority_worker()

# =============================================================================
# 3. IMPROVED TASK CANCELLATION
# =============================================================================
print("\n3. Testing Improved Task Cancellation")
print("-" * 60)

@mp.parallel
def long_running_task(duration):
    """A task that takes a long time"""
    time.sleep(duration)
    return f"Completed after {duration}s"

# Test basic cancellation
print("Testing basic cancellation...")
handle = long_running_task(5)
time.sleep(0.5)  # Let it start
print(f"Task started, elapsed time: {handle.elapsed_time():.2f}s")
print(f"Is cancelled: {handle.is_cancelled()}")
print(f"Task name: {handle.get_name()}")

handle.cancel()
print(f"Cancelled! Is cancelled: {handle.is_cancelled()}")

# Test cancellation with timeout
print("\nTesting cancellation with timeout...")
handle2 = long_running_task(10)
time.sleep(0.5)
success = handle2.cancel_with_timeout(2.0)
print(f"Cancellation {'succeeded' if success else 'timed out'}")

# =============================================================================
# 4. PERFORMANCE PROFILING
# =============================================================================
print("\n4. Testing Performance Profiling")
print("-" * 60)

# Reset metrics first
mp.reset_metrics()

@mp.profiled
def cpu_intensive(n):
    """CPU intensive task"""
    return sum(i**2 for i in range(n))

@mp.profiled
def quick_task(x):
    """Quick task"""
    return x * 2

# Run tasks multiple times
print("Running profiled tasks...")
for i in range(5):
    cpu_intensive(100000)
    quick_task(i)

# Add one failing task
@mp.profiled
def failing_task():
    """This task will fail"""
    raise ValueError("Intentional error")

try:
    failing_task()
except ValueError:
    pass

# Get all metrics
print("\nPerformance Metrics:")
all_metrics = mp.get_all_metrics()

for func_name, metrics in all_metrics.items():
    if not func_name.startswith('_global'):
        print(f"\n  {func_name}:")
        print(f"    Total tasks: {metrics['total_tasks']}")
        print(f"    Completed: {metrics['completed_tasks']}")
        print(f"    Failed: {metrics['failed_tasks']}")
        print(f"    Avg execution time: {metrics['average_execution_time_ms']:.2f}ms")
        print(f"    Total execution time: {metrics['total_execution_time_ms']:.2f}ms")

print(f"\nGlobal stats:")
print(f"  Total tasks: {all_metrics['_global_total']}")
print(f"  Completed: {all_metrics['_global_completed']}")
print(f"  Failed: {all_metrics['_global_failed']}")

# Test with parallel tasks
print("\n5. Testing Profiling with Parallel Tasks")
print("-" * 60)

mp.reset_metrics()

@mp.parallel
def parallel_cpu_task(n):
    """Parallel CPU task"""
    return sum(i**2 for i in range(n))

# Run multiple parallel tasks
handles = [parallel_cpu_task(500000) for _ in range(4)]
results = [h.get() for h in handles]

print(f"Completed {len(results)} parallel tasks")

# Get metrics
metrics = mp.get_all_metrics()
if 'parallel_cpu_task' in metrics:
    m = metrics['parallel_cpu_task']
    print(f"\nParallel task metrics:")
    print(f"  Total tasks: {m['total_tasks']}")
    print(f"  Average time: {m['average_execution_time_ms']:.2f}ms")

# =============================================================================
# 6. COMBINED FEATURES TEST
# =============================================================================
print("\n6. Combined Features Test")
print("-" * 60)

mp.reset_metrics()

@mp.parallel
def monitored_task(task_id, sleep_time):
    """Task with full monitoring"""
    time.sleep(sleep_time)
    return f"Task {task_id} done"

handles = []
for i in range(3):
    h = monitored_task(i, 0.5)
    handles.append(h)
    print(f"Started task {i}: {h.get_name()}")

# Monitor progress
while not all(h.is_ready() for h in handles):
    ready_count = sum(1 for h in handles if h.is_ready())
    print(f"  Progress: {ready_count}/{len(handles)} tasks completed", end='\r')
    time.sleep(0.1)

print(f"\n  All tasks completed!")

# Get results
for i, h in enumerate(handles):
    result = h.get()
    print(f"  {result} (took {h.elapsed_time():.2f}s)")

print("\n" + "=" * 60)
print("All tests completed successfully!")
print("=" * 60)
