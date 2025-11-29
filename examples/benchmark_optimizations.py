#!/usr/bin/env python3
"""
Benchmark: Original vs Optimized Implementations
Compares performance of different parallelization strategies
"""

import time
import statistics
from makeParallel import (
    parallel,           # Original (std::mpsc)
    parallel_fast,      # Crossbeam channels
    parallel_pool,      # Rayon thread pool
    memoize,            # Original (Arc<Mutex<HashMap>>)
    memoize_fast,       # DashMap (lock-free)
    parallel_map,       # Batch processing
)

print("=" * 80)
print("PERFORMANCE BENCHMARKS: Original vs Optimized")
print("=" * 80)

# =============================================================================
# BENCHMARK 1: Channel Performance (std::mpsc vs crossbeam)
# =============================================================================
print("\n" + "=" * 80)
print("BENCHMARK 1: Channel Performance")
print("=" * 80)

def cpu_work(n):
    """CPU-intensive task"""
    return sum(i**2 for i in range(n))

# Original - std::mpsc channels
@parallel
def task_original(n):
    return sum(i**2 for i in range(n))

# Optimized - crossbeam channels
@parallel_fast
def task_crossbeam(n):
    return sum(i**2 for i in range(n))

ITERATIONS = 100_000
NUM_TASKS = 100

print(f"\nTest: {NUM_TASKS} small tasks ({ITERATIONS:,} iterations each)")
print("-" * 80)

# Benchmark original
times_original = []
for _ in range(5):
    start = time.time()
    handles = [task_original(ITERATIONS) for _ in range(NUM_TASKS)]
    results = [h.get() for h in handles]
    elapsed = time.time() - start
    times_original.append(elapsed)

avg_original = statistics.mean(times_original)
std_original = statistics.stdev(times_original) if len(times_original) > 1 else 0

print(f"Original (std::mpsc):     {avg_original:.4f}s (±{std_original:.4f}s)")

# Benchmark crossbeam
times_crossbeam = []
for _ in range(5):
    start = time.time()
    handles = [task_crossbeam(ITERATIONS) for _ in range(NUM_TASKS)]
    results = [h.get() for h in handles]
    elapsed = time.time() - start
    times_crossbeam.append(elapsed)

avg_crossbeam = statistics.mean(times_crossbeam)
std_crossbeam = statistics.stdev(times_crossbeam) if len(times_crossbeam) > 1 else 0

print(f"Optimized (crossbeam):    {avg_crossbeam:.4f}s (±{std_crossbeam:.4f}s)")

speedup = avg_original / avg_crossbeam
print(f"\n✓ Speedup: {speedup:.2f}x faster with crossbeam channels")

# =============================================================================
# BENCHMARK 2: Thread Pool vs New Threads
# =============================================================================
print("\n" + "=" * 80)
print("BENCHMARK 2: Thread Pool Performance")
print("=" * 80)

@parallel
def task_new_thread(n):
    return sum(i**2 for i in range(n))

@parallel_pool
def task_pool(n):
    return sum(i**2 for i in range(n))

SMALL_TASK_SIZE = 50_000
NUM_SMALL_TASKS = 200

print(f"\nTest: {NUM_SMALL_TASKS} small tasks ({SMALL_TASK_SIZE:,} iterations)")
print("(Thread pool should excel at many small tasks)")
print("-" * 80)

# New thread per task
times_newthread = []
for _ in range(3):
    start = time.time()
    handles = [task_new_thread(SMALL_TASK_SIZE) for _ in range(NUM_SMALL_TASKS)]
    results = [h.get() for h in handles]
    elapsed = time.time() - start
    times_newthread.append(elapsed)

avg_newthread = statistics.mean(times_newthread)

print(f"New thread per task:  {avg_newthread:.4f}s")

# Thread pool (rayon)
times_pool = []
for _ in range(3):
    start = time.time()
    handles = [task_pool(SMALL_TASK_SIZE) for _ in range(NUM_SMALL_TASKS)]
    results = [h.get() for h in handles]
    elapsed = time.time() - start
    times_pool.append(elapsed)

avg_pool = statistics.mean(times_pool)

print(f"Thread pool (rayon):  {avg_pool:.4f}s")

speedup_pool = avg_newthread / avg_pool
print(f"\n✓ Speedup: {speedup_pool:.2f}x faster with thread pool")
print(f"  Thread pool avoids overhead of spawning {NUM_SMALL_TASKS} OS threads")

# =============================================================================
# BENCHMARK 3: Memoize Performance (Mutex vs DashMap)
# =============================================================================
print("\n" + "=" * 80)
print("BENCHMARK 3: Memoize - Lock-Free vs Mutex")
print("=" * 80)

def expensive_calc(x):
    """Simulate expensive calculation"""
    return sum(i**2 for i in range(x))

@memoize
def cached_original(x):
    return expensive_calc(x)

@memoize_fast
def cached_dashmap(x):
    return expensive_calc(x)

# Test with repeated calls (high cache hit rate)
test_values = [1000, 2000, 3000, 1000, 2000, 3000] * 50  # Repeated values

print(f"\nTest: {len(test_values)} calls with repeated values")
print("(High cache hit rate - lock-free should excel)")
print("-" * 80)

# Original (Arc<Mutex<HashMap>>)
start = time.time()
for val in test_values:
    _ = cached_original(val)
time_mutex = time.time() - start

print(f"Original (Mutex):     {time_mutex:.4f}s")

# DashMap (lock-free)
start = time.time()
for val in test_values:
    _ = cached_dashmap(val)
time_dashmap = time.time() - start

print(f"Optimized (DashMap):  {time_dashmap:.4f}s")

speedup_cache = time_mutex / time_dashmap
print(f"\n✓ Speedup: {speedup_cache:.2f}x faster with lock-free DashMap")
print(f"  Lock-free reads eliminate contention on cache hits")

# =============================================================================
# BENCHMARK 4: Batch Processing (parallel_map)
# =============================================================================
print("\n" + "=" * 80)
print("BENCHMARK 4: Batch Processing with parallel_map")
print("=" * 80)

def process_item(x):
    """Process single item"""
    return sum(i**2 for i in range(x))

items = [50_000] * 100

print(f"\nTest: Process {len(items)} items")
print("-" * 80)

# Individual parallel calls
@parallel_fast
def process_parallel(x):
    return process_item(x)

start = time.time()
handles = [process_parallel(x) for x in items]
results = [h.get() for h in handles]
time_individual = time.time() - start

print(f"Individual calls:  {time_individual:.4f}s")

# Batch with parallel_map
start = time.time()
results = parallel_map(lambda x: process_item(x), items)
time_batch = time.time() - start

print(f"Batch (parallel_map): {time_batch:.4f}s")

speedup_batch = time_individual / time_batch
print(f"\n✓ Speedup: {speedup_batch:.2f}x faster with batch processing")
print(f"  Rayon's work-stealing scheduler optimizes load balancing")

# =============================================================================
# SUMMARY TABLE
# =============================================================================
print("\n" + "=" * 80)
print("PERFORMANCE SUMMARY")
print("=" * 80)

print(f"""
┌─────────────────────────────────┬──────────────┬──────────────┬──────────┐
│ Optimization                    │ Original     │ Optimized    │ Speedup  │
├─────────────────────────────────┼──────────────┼──────────────┼──────────┤
│ 1. Crossbeam Channels           │ {avg_original:>10.4f}s │ {avg_crossbeam:>10.4f}s │ {speedup:>6.2f}x │
│ 2. Thread Pool (Rayon)          │ {avg_newthread:>10.4f}s │ {avg_pool:>10.4f}s │ {speedup_pool:>6.2f}x │
│ 3. Lock-Free Cache (DashMap)    │ {time_mutex:>10.4f}s │ {time_dashmap:>10.4f}s │ {speedup_cache:>6.2f}x │
│ 4. Batch Processing (par_iter)  │ {time_individual:>10.4f}s │ {time_batch:>10.4f}s │ {speedup_batch:>6.2f}x │
└─────────────────────────────────┴──────────────┴──────────────┴──────────┘

KEY IMPROVEMENTS:
  ✓ Crossbeam:    Lock-free MPMC channels, better performance
  ✓ Rayon:        Work-stealing thread pool, reduces overhead
  ✓ DashMap:      Lock-free concurrent HashMap, eliminates contention
  ✓ parallel_map: Batch processing with optimal load balancing

WHEN TO USE EACH:
  • parallel       : General purpose, simple tasks
  • parallel_fast  : High-throughput, many concurrent tasks
  • parallel_pool  : Many small tasks, reduce thread spawn overhead
  • parallel_map   : Batch processing, automatic load balancing
  • memoize_fast   : High cache hit rate, concurrent access
""")

print("=" * 80)
print("Recommendation: Use optimized versions for production workloads")
print("=" * 80)
