"""
Comprehensive showcase of all makeParallel features including new additions
"""

import time
import makeParallel as mp

print("\n" + "=" * 70)
print("makeParallel - Complete Feature Showcase")
print("=" * 70)

# =============================================================================
# SETUP AND CONFIGURATION
# =============================================================================

print("\n📊 SETUP: Thread Pool Configuration")
print("-" * 70)

# Configure thread pool for optimal performance
mp.configure_thread_pool(num_threads=8)
info = mp.get_thread_pool_info()
print(f"✓ Thread pool configured with {info['current_num_threads']} threads")

# Reset metrics for clean demonstration
mp.reset_metrics()
print("✓ Performance metrics reset")

# =============================================================================
# BASIC DECORATORS
# =============================================================================

print("\n📦 BASIC DECORATORS")
print("-" * 70)

# Timer - Measure execution time
@mp.timer
def timed_computation(n):
    """Example with @timer decorator"""
    return sum(i**2 for i in range(n))

print("\n1. @timer - Automatic timing:")
result = timed_computation(100000)
print(f"   Result: {result}")

# Memoization - Cache results
@mp.memoize
def expensive_calculation(x, y):
    """Example with @memoize decorator"""
    print(f"   Computing {x} ** {y}...")
    return x ** y

print("\n2. @memoize - Caching:")
print("   First call:")
result1 = expensive_calculation(2, 20)
print("   Second call (cached):")
result2 = expensive_calculation(2, 20)
print(f"   Results match: {result1 == result2}")

# Call Counter
@mp.CallCounter
def tracked_function(x):
    """Example with @CallCounter decorator"""
    return x * 2

print("\n3. @CallCounter - Call tracking:")
for i in range(5):
    tracked_function(i)
print(f"   Function called {tracked_function.call_count} times")

# =============================================================================
# PARALLEL EXECUTION
# =============================================================================

print("\n⚡ PARALLEL EXECUTION")
print("-" * 70)

@mp.parallel
def cpu_bound_task(task_id, duration):
    """CPU-intensive task running in parallel"""
    time.sleep(duration)
    return f"Task {task_id} completed"

print("\n4. @parallel - True parallelism:")
print("   Launching 4 parallel tasks...")
start = time.time()
handles = [cpu_bound_task(i, 0.5) for i in range(4)]

# Monitor progress
while not all(h.is_ready() for h in handles):
    ready = sum(1 for h in handles if h.is_ready())
    print(f"   Progress: {ready}/4 tasks complete", end='\r')
    time.sleep(0.1)

results = [h.get() for h in handles]
elapsed = time.time() - start
print(f"\n   ✓ All 4 tasks completed in {elapsed:.2f}s (would take 2.0s sequentially)")

# =============================================================================
# PRIORITY QUEUES (NEW!)
# =============================================================================

print("\n🎯 PRIORITY QUEUES (NEW!)")
print("-" * 70)

@mp.parallel_priority
def priority_task(name, level):
    """Task with priority-based execution"""
    time.sleep(0.1)
    return f"{name} (priority {level})"

print("\n5. @parallel_priority - Priority-based scheduling:")
mp.start_priority_worker()

# Submit tasks with different priorities
print("   Submitting tasks:")
tasks = [
    ("Low priority", 1),
    ("Medium priority", 50),
    ("High priority", 100),
    ("Critical priority", 200),
]

priority_handles = []
for name, priority in tasks:
    print(f"     - {name}: priority={priority}")
    handle = priority_task(name, priority, priority=priority)
    priority_handles.append(handle)

print("\n   Execution order (by priority):")
for h in priority_handles:
    result = h.get()
    print(f"     ✓ {result}")

mp.stop_priority_worker()

# =============================================================================
# ENHANCED CANCELLATION (NEW!)
# =============================================================================

print("\n🛑 ENHANCED CANCELLATION (NEW!)")
print("-" * 70)

@mp.parallel
def long_running_task(duration):
    """A task that can be cancelled"""
    time.sleep(duration)
    return f"Completed {duration}s task"

print("\n6. Enhanced cancellation controls:")

# Start a long task
handle = long_running_task(10)
time.sleep(0.2)  # Let it start

print(f"   Task: {handle.get_name()}")
print(f"   Elapsed time: {handle.elapsed_time():.2f}s")
print(f"   Is cancelled: {handle.is_cancelled()}")

# Cancel with timeout
print("   Cancelling task...")
if handle.cancel_with_timeout(1.0):
    print(f"   ✓ Successfully cancelled")
    print(f"   Is cancelled now: {handle.is_cancelled()}")
else:
    print(f"   ✗ Cancellation timed out")

# =============================================================================
# PERFORMANCE PROFILING (NEW!)
# =============================================================================

print("\n📈 PERFORMANCE PROFILING (NEW!)")
print("-" * 70)

mp.reset_metrics()

@mp.profiled
def profiled_task(n):
    """Task with automatic profiling"""
    total = 0
    for i in range(n):
        total += i ** 2
    return total

print("\n7. @profiled - Automatic performance tracking:")
print("   Running 10 iterations...")

# Run multiple times
for i in range(10):
    profiled_task(50000)

# Get metrics
metrics = mp.get_metrics("profiled_task")
if metrics:
    print(f"\n   Performance Metrics:")
    print(f"     Total executions: {metrics.total_tasks}")
    print(f"     Successful: {metrics.completed_tasks}")
    print(f"     Failed: {metrics.failed_tasks}")
    print(f"     Average time: {metrics.average_execution_time_ms:.2f}ms")
    print(f"     Total time: {metrics.total_execution_time_ms:.2f}ms")

# Test with parallel tasks (auto-profiled!)
print("\n8. Parallel tasks are auto-profiled:")

@mp.parallel
def auto_profiled_parallel(task_id):
    """Parallel tasks are automatically profiled"""
    return sum(i**2 for i in range(100000))

handles = [auto_profiled_parallel(i) for i in range(5)]
results = [h.get() for h in handles]

# Check metrics
all_metrics = mp.get_all_metrics()
if 'auto_profiled_parallel' in all_metrics:
    m = all_metrics['auto_profiled_parallel']
    print(f"   Parallel task metrics:")
    print(f"     Tasks executed: {m['total_tasks']}")
    print(f"     Average time: {m['average_execution_time_ms']:.2f}ms")

# =============================================================================
# COMBINED USAGE
# =============================================================================

print("\n🔥 COMBINED USAGE - All Features Together")
print("-" * 70)

mp.reset_metrics()

@mp.parallel
@mp.profiled
def ultimate_task(task_id, compute_size):
    """Task using both parallel execution and profiling"""
    result = sum(i**2 for i in range(compute_size))
    return f"Task {task_id}: {result}"

print("\n9. Combining @parallel + @profiled:")
print("   Running 6 parallel profiled tasks...")

handles = [ultimate_task(i, 200000) for i in range(6)]

# Monitor with new cancellation features
all_ready = False
while not all_ready:
    ready_count = sum(1 for h in handles if h.is_ready())
    if ready_count > 0:
        elapsed = handles[0].elapsed_time()
        print(f"   Progress: {ready_count}/6 tasks, elapsed: {elapsed:.2f}s", end='\r')

    all_ready = all(h.is_ready() for h in handles)
    time.sleep(0.1)

print("\n   ✓ All tasks completed!")

# Get detailed metrics
metrics = mp.get_all_metrics()
if 'ultimate_task' in metrics:
    m = metrics['ultimate_task']
    print(f"\n   Final Metrics:")
    print(f"     Total tasks: {m['total_tasks']}")
    print(f"     Success rate: {m['completed_tasks']/m['total_tasks']*100:.0f}%")
    print(f"     Average time: {m['average_execution_time_ms']:.2f}ms")

# =============================================================================
# SUMMARY
# =============================================================================

print("\n" + "=" * 70)
print("📊 GLOBAL STATISTICS")
print("=" * 70)

all_metrics = mp.get_all_metrics()
print(f"\nTotal tasks executed: {all_metrics['_global_total']}")
print(f"Successfully completed: {all_metrics['_global_completed']}")
print(f"Failed tasks: {all_metrics['_global_failed']}")
print(f"Success rate: {all_metrics['_global_completed']/all_metrics['_global_total']*100:.1f}%")

print("\n" + "=" * 70)
print("✅ Feature Showcase Complete!")
print("=" * 70)

print("\n📚 Features Demonstrated:")
print("   ✓ Thread pool configuration")
print("   ✓ Basic decorators (@timer, @memoize, @CallCounter)")
print("   ✓ Parallel execution (@parallel)")
print("   ✓ Priority queues (@parallel_priority)")
print("   ✓ Enhanced cancellation (cancel_with_timeout, is_cancelled, etc.)")
print("   ✓ Performance profiling (@profiled, get_metrics)")
print("   ✓ Combined feature usage")

print("\n📖 For more information, see:")
print("   - docs/NEW_FEATURES.md for detailed documentation")
print("   - README.md for quick start guide")
print("   - examples/ for more examples")
print()
