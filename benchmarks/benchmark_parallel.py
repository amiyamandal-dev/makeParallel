#!/usr/bin/env python3
"""
Performance benchmarks for makeParallel
Compares makeParallel with threading and multiprocessing
"""

import time
import multiprocessing as mp_std
import threading
import sys
import os
from contextlib import contextmanager

sys.path.insert(0, '.')

try:
    import makeparallel as mp
except ImportError:
    print("Error: makeparallel not installed. Run 'maturin develop' first.")
    sys.exit(1)


@contextmanager
def suppress_stdout():
    """Suppress stdout temporarily"""
    with open(os.devnull, 'w') as devnull:
        old_stdout = sys.stdout
        sys.stdout = devnull
        try:
            yield
        finally:
            sys.stdout = old_stdout


def cpu_intensive_task(n):
    """CPU-bound task for benchmarking"""
    result = 0
    for i in range(n):
        result += i * i
    return result


def io_simulation_task(duration):
    """Simulates I/O with sleep"""
    time.sleep(duration)
    return "done"


class Benchmark:
    def __init__(self, name):
        self.name = name
        self.results = {}

    def run(self, func, *args, iterations=1, **kwargs):
        """Run benchmark and return average time"""
        times = []
        for _ in range(iterations):
            start = time.time()
            result = func(*args, **kwargs)
            elapsed = time.time() - start
            times.append(elapsed)
        
        avg_time = sum(times) / len(times)
        self.results[func.__name__] = avg_time
        return avg_time

    def print_results(self):
        """Print formatted results"""
        print(f"\n{'='*70}")
        print(f"Benchmark: {self.name}")
        print(f"{'='*70}")
        
        if not self.results:
            print("No results to display")
            return

        # Find baseline (usually first result)
        baseline_name = list(self.results.keys())[0]
        baseline_time = self.results[baseline_name]

        for name, time_taken in self.results.items():
            speedup = baseline_time / time_taken if time_taken > 0 else 0
            print(f"{name:40} {time_taken:8.4f}s  ({speedup:5.2f}x)")

        print(f"{'='*70}\n")


def benchmark_cpu_intensive():
    """Benchmark CPU-intensive tasks"""
    bench = Benchmark("CPU-Intensive Tasks (10 tasks, 1M iterations each)")

    num_tasks = 10
    iterations = 1_000_000

    # Sequential baseline
    def sequential():
        results = [cpu_intensive_task(iterations) for _ in range(num_tasks)]
        return results

    # makeParallel @parallel
    @mp.parallel
    def mp_parallel_task(n):
        return cpu_intensive_task(n)

    def using_makeparallel_parallel():
        handles = [mp_parallel_task(iterations) for _ in range(num_tasks)]
        return [h.get() for h in handles]

    # makeParallel @parallel_pool
    @mp.parallel_pool
    def mp_pool_task(n):
        return cpu_intensive_task(n)

    def using_makeparallel_pool():
        handles = [mp_pool_task(iterations) for _ in range(num_tasks)]
        return [h.get() for h in handles]

    # makeParallel parallel_map
    def using_makeparallel_map():
        return mp.parallel_map(cpu_intensive_task, [iterations] * num_tasks)

    # Python multiprocessing
    def using_multiprocessing():
        with mp_std.Pool(num_tasks) as pool:
            return pool.map(cpu_intensive_task, [iterations] * num_tasks)

    # Python threading (won't speed up CPU-bound due to GIL)
    def using_threading():
        results = [None] * num_tasks
        threads = []
        
        def worker(index, n):
            results[index] = cpu_intensive_task(n)
        
        for i in range(num_tasks):
            t = threading.Thread(target=worker, args=(i, iterations))
            t.start()
            threads.append(t)
        
        for t in threads:
            t.join()
        
        return results

    print("Running CPU-intensive benchmarks (this may take a minute)...")

    print("  Testing sequential...")
    with suppress_stdout():
        bench.run(sequential)

    print("  Testing makeParallel @parallel...")
    with suppress_stdout():
        bench.run(using_makeparallel_parallel)

    print("  Testing makeParallel @parallel_pool...")
    with suppress_stdout():
        bench.run(using_makeparallel_pool)

    print("  Testing makeParallel parallel_map...")
    with suppress_stdout():
        bench.run(using_makeparallel_map)

    print("  Testing multiprocessing...")
    with suppress_stdout():
        bench.run(using_multiprocessing)

    print("  Testing threading...")
    with suppress_stdout():
        bench.run(using_threading)

    bench.print_results()


def benchmark_decorator_overhead():
    """Benchmark decorator overhead"""
    bench = Benchmark("Decorator Overhead (10,000 calls)")

    iterations = 10_000

    def plain_function(x):
        return x * 2

    @mp.CallCounter
    def with_counter(x):
        return x * 2

    @mp.memoize
    def with_memoize(x):
        return x * 2

    @mp.memoize_fast
    def with_memoize_fast(x):
        return x * 2

    # Reset metrics
    mp.reset_metrics()

    print("Running decorator overhead benchmarks...")
    print("  (Comparing plain function vs. decorators)")

    # Plain function baseline
    def test_plain():
        for i in range(iterations):
            plain_function(i % 100)

    def test_counter():
        for i in range(iterations):
            with_counter(i % 100)

    def test_memoize():
        for i in range(iterations):
            with_memoize(i % 100)  # Will cache 100 unique values

    def test_memoize_fast():
        for i in range(iterations):
            with_memoize_fast(i % 100)

    with suppress_stdout():
        bench.run(test_plain)
        bench.run(test_counter)
        bench.run(test_memoize)
        bench.run(test_memoize_fast)

    bench.print_results()


def benchmark_retry_caching():
    """Benchmark retry with caching decorator"""
    bench = Benchmark("Retry with Caching (100 successful cached calls)")

    call_count = [0]

    @mp.retry_cached(max_attempts=3)
    def sometimes_fails(x):
        call_count[0] += 1
        if x < 100:  # All succeed for benchmark
            return x * 2
        raise ValueError(f"Failed for {x}")

    def test_cached_successes():
        # Call twice - second time should be cached
        results1 = []
        results2 = []
        for i in range(100):
            results1.append(sometimes_fails(i))
        for i in range(100):
            results2.append(sometimes_fails(i))
        return results1, results2

    print("Running retry/caching benchmarks...")
    print("  (Testing caching effectiveness - no errors expected)")

    with suppress_stdout():
        bench.run(test_cached_successes)

    bench.print_results()

    print(f"\nCache efficiency:")
    print(f"  Total function calls: {call_count[0]}")
    print(f"  Expected with caching: 100 (first call of each value)")
    print(f"  Without caching would be: 200 (100 values × 2 iterations)")
    print(f"  Cache hit rate: {((200 - call_count[0]) / 200 * 100):.1f}%")


def main():
    """Run all benchmarks"""
    print("\n" + "="*70)
    print(" makeParallel Performance Benchmarks")
    print("="*70)

    try:
        # CPU-intensive benchmark
        benchmark_cpu_intensive()

        # Decorator overhead
        benchmark_decorator_overhead()

        # Retry caching
        benchmark_retry_caching()

        print("\nAll benchmarks completed successfully!")

    except Exception as e:
        print(f"\nError during benchmarking: {e}")
        import traceback
        traceback.print_exc()
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
