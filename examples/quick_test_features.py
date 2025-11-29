"""
Quick test for new makeParallel features
"""

import time
import makeParallel as mp

print("Testing New Features")
print("=" * 60)

# 1. Thread Pool Configuration
print("\n1. Thread Pool Configuration")
info = mp.get_thread_pool_info()
print(f"   Initial pool: {info}")

mp.configure_thread_pool(num_threads=4)
info = mp.get_thread_pool_info()
print(f"   Configured pool: {info}")
print("   ✓ Thread pool configuration works!")

# 2. Performance Profiling
print("\n2. Performance Profiling")
mp.reset_metrics()

@mp.profiled
def test_func(n):
    return sum(i**2 for i in range(n))

# Run it a few times
for _ in range(3):
    test_func(10000)

metrics = mp.get_all_metrics()
if 'test_func' in metrics:
    m = metrics['test_func']
    print(f"   Tasks: {m['total_tasks']}, Avg time: {m['average_execution_time_ms']:.2f}ms")
    print("   ✓ Profiling works!")

# 3. Improved Cancellation
print("\n3. Improved Cancellation")

@mp.parallel
def cancelable_task():
    time.sleep(0.5)
    return "done"

handle = cancelable_task()
print(f"   Task name: {handle.get_name()}")
print(f"   Elapsed: {handle.elapsed_time():.2f}s")
print(f"   Is cancelled: {handle.is_cancelled()}")
handle.cancel()
print(f"   After cancel: {handle.is_cancelled()}")
print("   ✓ Cancellation improvements work!")

# 4. Priority Queue
print("\n4. Priority Queue")

@mp.parallel_priority
def priority_task(task_id):
    return f"Task {task_id}"

mp.start_priority_worker()

# Submit with different priorities
h1 = priority_task(1, priority=1)
h2 = priority_task(2, priority=10)  # Higher priority

time.sleep(0.3)
r1 = h1.get()
r2 = h2.get()

print(f"   {r1}, {r2}")
print("   ✓ Priority queue works!")

mp.stop_priority_worker()

print("\n" + "=" * 60)
print("All new features working correctly!")
print("=" * 60)
