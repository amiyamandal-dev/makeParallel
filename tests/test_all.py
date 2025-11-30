#!/usr/bin/env python3
"""
Comprehensive Test Suite for makeParallel
Tests all decorators and functions to ensure they work as expected
"""

import sys
import time

import makeparallel as mp


class TestRunner:
    def __init__(self):
        self.passed = 0
        self.failed = 0
        self.tests = []

    def test(self, name):
        """Decorator to mark test functions"""

        def decorator(func):
            self.tests.append((name, func))
            return func

        return decorator

    def assert_equal(self, actual, expected, msg=""):
        if actual != expected:
            raise AssertionError(f"{msg}\nExpected: {expected}\nGot: {actual}")

    def assert_true(self, condition, msg=""):
        if not condition:
            raise AssertionError(f"{msg}\nCondition is False")

    def assert_raises(self, exception_type, func):
        try:
            func()
            raise AssertionError(
                f"Expected {exception_type.__name__} but no exception was raised"
            )
        except exception_type:
            pass

    def run(self):
        print("=" * 80)
        print("COMPREHENSIVE TEST SUITE - makeParallel")
        print("=" * 80)

        for name, test_func in self.tests:
            try:
                print(f"\n[TEST] {name}...", end=" ")
                test_func(self)
                print(" PASSED")
                self.passed += 1
            except Exception as e:
                print(f" FAILED")
                print(f"  Error: {e}")
                self.failed += 1

        print("\n" + "=" * 80)
        print(f"RESULTS: {self.passed} passed, {self.failed} failed")
        print("=" * 80)

        return self.failed == 0


# Create test runner
runner = TestRunner()


# =============================================================================
# TEST 1: Timer Decorator
# =============================================================================
@runner.test("Timer - Basic functionality")
def test_timer_basic(t):
    @mp.timer
    def slow_func():
        time.sleep(0.1)
        return 42

    result = slow_func()
    t.assert_equal(result, 42)


@runner.test("Timer - With arguments")
def test_timer_args(t):
    @mp.timer
    def add(a, b):
        return a + b

    result = add(5, 3)
    t.assert_equal(result, 8)


# =============================================================================
# TEST 2: Log Calls Decorator
# =============================================================================
@runner.test("Log Calls - Basic functionality")
def test_log_calls_basic(t):
    @mp.log_calls
    def multiply(x, y):
        return x * y

    result = multiply(3, 4)
    t.assert_equal(result, 12)


@runner.test("Log Calls - With kwargs")
def test_log_calls_kwargs(t):
    @mp.log_calls
    def greet(name, greeting="Hello"):
        return f"{greeting}, {name}!"

    result = greet("World", greeting="Hi")
    t.assert_equal(result, "Hi, World!")


# =============================================================================
# TEST 3: CallCounter Decorator
# =============================================================================
@runner.test("CallCounter - Count tracking")
def test_callcounter_basic(t):
    @mp.CallCounter
    def counted_func():
        return "called"

    counted_func()
    counted_func()
    counted_func()

    t.assert_equal(counted_func.call_count, 3)


@runner.test("CallCounter - Reset functionality")
def test_callcounter_reset(t):
    @mp.CallCounter
    def func():
        return 1

    func()
    func()
    t.assert_equal(func.call_count, 2)

    func.reset()
    t.assert_equal(func.call_count, 0)

    func()
    t.assert_equal(func.call_count, 1)


@runner.test("CallCounter - With arguments")
def test_callcounter_args(t):
    @mp.CallCounter
    def add(a, b):
        return a + b

    result1 = add(1, 2)
    result2 = add(3, 4)

    t.assert_equal(result1, 3)
    t.assert_equal(result2, 7)
    t.assert_equal(add.call_count, 2)


# =============================================================================
# TEST 4: Retry Decorator
# =============================================================================
@runner.test("Retry - Successful after retries")
def test_retry_success(t):
    attempts = [0]

    @mp.retry(max_retries=3)
    def flaky():
        attempts[0] += 1
        if attempts[0] < 3:
            raise ValueError("Not yet!")
        return "success"

    result = flaky()
    t.assert_equal(result, "success")
    t.assert_equal(attempts[0], 3)


@runner.test("Retry - Fails after max retries")
def test_retry_failure(t):
    @mp.retry(max_retries=2)
    def always_fails():
        raise RuntimeError("Always fails")

    t.assert_raises(RuntimeError, always_fails)


@runner.test("Retry - Immediate success")
def test_retry_immediate(t):
    @mp.retry(max_retries=3)
    def immediate():
        return 42

    result = immediate()
    t.assert_equal(result, 42)


# =============================================================================
# TEST 5: Memoize Decorator
# =============================================================================
@runner.test("Memoize - Caching works")
def test_memoize_caching(t):
    call_count = [0]

    @mp.memoize
    def expensive(x):
        call_count[0] += 1
        return x**2

    # First call - cache miss
    result1 = expensive(5)
    t.assert_equal(result1, 25)
    t.assert_equal(call_count[0], 1)

    # Second call - cache hit
    result2 = expensive(5)
    t.assert_equal(result2, 25)
    t.assert_equal(call_count[0], 1)  # No additional call

    # Different argument - cache miss
    result3 = expensive(6)
    t.assert_equal(result3, 36)
    t.assert_equal(call_count[0], 2)


@runner.test("Memoize - With kwargs")
def test_memoize_kwargs(t):
    @mp.memoize
    def power(base, exp=2):
        return base**exp

    result1 = power(2, exp=3)
    result2 = power(2, exp=3)
    result3 = power(2)

    t.assert_equal(result1, 8)
    t.assert_equal(result2, 8)
    t.assert_equal(result3, 4)


# =============================================================================
# TEST 6: Parallel Decorator
# =============================================================================
@runner.test("Parallel - Basic functionality")
def test_parallel_basic(t):
    @mp.parallel
    def compute(x):
        return x * 2

    handle = compute(21)
    result = handle.get()
    t.assert_equal(result, 42)


@runner.test("Parallel - is_ready() check")
def test_parallel_ready(t):
    @mp.parallel
    def slow_task():
        time.sleep(0.2)
        return "done"

    handle = slow_task()

    # Should not be ready immediately
    ready_before = handle.is_ready()

    # Get result (blocks)
    result = handle.get()

    # Should be ready after get()
    ready_after = handle.is_ready()

    t.assert_equal(ready_before, False)
    t.assert_equal(result, "done")
    t.assert_equal(ready_after, True)


@runner.test("Parallel - try_get() non-blocking")
def test_parallel_try_get(t):
    @mp.parallel
    def instant():
        return 123

    handle = instant()
    time.sleep(0.1)  # Give it time to complete

    result = handle.try_get()
    t.assert_equal(result, 123)


@runner.test("Parallel - Multiple tasks")
def test_parallel_multiple(t):
    @mp.parallel
    def square(x):
        return x**2

    handles = [square(i) for i in range(5)]
    results = [h.get() for h in handles]

    expected = [0, 1, 4, 9, 16]
    t.assert_equal(results, expected)


@runner.test("Parallel - Error handling")
def test_parallel_error(t):
    @mp.parallel
    def failing():
        raise ValueError("Test error")

    handle = failing()
    t.assert_raises(Exception, handle.get)


@runner.test("Parallel - With args and kwargs")
def test_parallel_args_kwargs(t):
    @mp.parallel
    def calc(a, b, multiplier=1):
        return (a + b) * multiplier

    handle = calc(3, 4, multiplier=2)
    result = handle.get()
    t.assert_equal(result, 14)


# =============================================================================
# TEST 7: Parallel Fast (Crossbeam)
# =============================================================================
@runner.test("Parallel Fast - Basic functionality")
def test_parallel_fast_basic(t):
    @mp.parallel_fast
    def compute(x):
        return x * 3

    handle = compute(7)
    result = handle.get()
    t.assert_equal(result, 21)


@runner.test("Parallel Fast - Multiple concurrent tasks")
def test_parallel_fast_concurrent(t):
    @mp.parallel_fast
    def task(x):
        return x**2

    handles = [task(i) for i in range(10)]
    results = [h.get() for h in handles]

    expected = [i**2 for i in range(10)]
    t.assert_equal(results, expected)


# =============================================================================
# TEST 8: Parallel Pool (Rayon)
# =============================================================================
@runner.test("Parallel Pool - Basic functionality")
def test_parallel_pool_basic(t):
    @mp.parallel_pool
    def compute(x):
        return x + 10

    handle = compute(5)
    result = handle.get()
    t.assert_equal(result, 15)


@runner.test("Parallel Pool - Many small tasks")
def test_parallel_pool_many(t):
    @mp.parallel_pool
    def small_task(x):
        return x * 2

    # Spawn many small tasks
    handles = [small_task(i) for i in range(50)]
    results = [h.get() for h in handles]

    expected = [i * 2 for i in range(50)]
    t.assert_equal(results, expected)


# =============================================================================
# TEST 9: Memoize Fast (DashMap)
# =============================================================================
@runner.test("Memoize Fast - Caching works")
def test_memoize_fast_caching(t):
    call_count = [0]

    @mp.memoize_fast
    def expensive(x):
        call_count[0] += 1
        return x**3

    result1 = expensive(3)
    result2 = expensive(3)
    result3 = expensive(4)

    t.assert_equal(result1, 27)
    t.assert_equal(result2, 27)
    t.assert_equal(result3, 64)
    t.assert_equal(call_count[0], 2)  # Only 2 actual calls


# =============================================================================
# TEST 10: Parallel Map (Batch Processing)
# =============================================================================
@runner.test("Parallel Map - Basic batch processing")
def test_parallel_map_basic(t):
    def square(x):
        return x**2

    items = list(range(10))
    results = mp.parallel_map(square, items)

    expected = [i**2 for i in range(10)]
    t.assert_equal(results, expected)


@runner.test("Parallel Map - Large batch")
def test_parallel_map_large(t):
    def double(x):
        return x * 2

    items = list(range(100))
    results = mp.parallel_map(double, items)

    expected = [i * 2 for i in range(100)]
    t.assert_equal(results, expected)


# =============================================================================
# TEST 11: Class Methods
# =============================================================================
@runner.test("Timer - On class method")
def test_timer_class_method(t):
    class Calculator:
        @mp.timer
        def add(self, a, b):
            return a + b

    calc = Calculator()
    result = calc.add(10, 20)
    t.assert_equal(result, 30)


@runner.test("CallCounter - On class method")
def test_callcounter_class_method(t):
    class Counter:
        @mp.CallCounter
        def method(self, x):
            return x * 2

    obj = Counter()
    obj.method(5)
    obj.method(6)

    t.assert_equal(obj.method.call_count, 2)


@runner.test("Parallel - On class method")
def test_parallel_class_method(t):
    class Worker:
        def __init__(self, factor):
            self.factor = factor

        @mp.parallel
        def process(self, x):
            return x * self.factor

    worker = Worker(3)
    handle = worker.process(7)
    result = handle.get()
    t.assert_equal(result, 21)


# =============================================================================
# TEST 12: Combined Decorators
# =============================================================================
@runner.test("Combined - Timer + Log")
def test_combined_timer_log(t):
    @mp.timer
    @mp.log_calls
    def combined(x):
        return x + 1

    result = combined(5)
    t.assert_equal(result, 6)


@runner.test("Combined - Memoize + Timer")
def test_combined_memoize_timer(t):
    @mp.memoize
    @mp.timer
    def cached_slow(x):
        return x**2

    result1 = cached_slow(5)
    result2 = cached_slow(5)

    t.assert_equal(result1, 25)
    t.assert_equal(result2, 25)


# =============================================================================
# TEST 13: Edge Cases
# =============================================================================
@runner.test("Edge Case - Empty arguments")
def test_edge_empty_args(t):
    @mp.parallel
    def no_args():
        return "success"

    handle = no_args()
    result = handle.get()
    t.assert_equal(result, "success")


@runner.test("Edge Case - None return value")
def test_edge_none_return(t):
    @mp.parallel
    def returns_none():
        return None

    handle = returns_none()
    result = handle.get()
    t.assert_equal(result, None)


@runner.test("Edge Case - Large data structure")
def test_edge_large_data(t):
    @mp.parallel
    def create_list(n):
        return list(range(n))

    handle = create_list(1000)
    result = handle.get()
    t.assert_equal(len(result), 1000)
    t.assert_equal(result[0], 0)
    t.assert_equal(result[-1], 999)


# =============================================================================
# TEST 14: Advanced Features
# =============================================================================
@runner.test("Advanced - AsyncHandle.cancel()")
def test_advanced_cancel(t):
    @mp.parallel
    def long_running_task():
        time.sleep(2)
        return "should not complete"

    handle = long_running_task()
    time.sleep(0.1)
    handle.cancel()

    t.assert_true(handle.is_cancelled(), "handle.is_cancelled() should be True.")
    # Note: Once cancelled, the task is marked complete and get() will return the cached error
    # Since we can't interrupt Python's time.sleep(), the thread continues but is marked cancelled
    t.assert_true(handle.is_ready(), "handle.is_ready() should be True after cancel.")


@runner.test("Advanced - Task timeout")
def test_advanced_timeout(t):
    @mp.parallel
    def task_that_will_timeout():
        time.sleep(1)
        return "should have timed out"

    handle = task_that_will_timeout(timeout=0.5)
    t.assert_raises(Exception, handle.get)


@runner.test("Advanced - Task metadata")
def test_advanced_metadata(t):
    @mp.parallel
    def task_with_metadata(x):
        return x

    handle = task_with_metadata(123)
    handle.set_metadata("user_id", "user-abc")
    handle.set_metadata("request_id", "req-123")
    metadata = handle.get_all_metadata()

    t.assert_equal(metadata.get("user_id"), "user-abc")
    t.assert_equal(metadata.get("request_id"), "req-123")


@runner.test("Advanced - Thread pool configuration")
def test_advanced_threadpool_config(t):
    mp.configure_thread_pool(num_threads=4)
    info = mp.get_thread_pool_info()
    t.assert_true(info["configured"])
    # Note: num_threads info may vary based on implementation


@runner.test("Advanced - @mp.parallel_priority")
def test_advanced_priority(t):
    # Start the priority worker
    mp.start_priority_worker()

    @mp.parallel_priority
    def priority_task(value):
        time.sleep(0.1)
        return value

    # Submit tasks with different priorities
    # Higher priority values should execute first
    low_prio_handle = priority_task(1, priority=1)
    medium_prio_handle = priority_task(5, priority=5)
    high_prio_handle = priority_task(10, priority=10)

    # Wait for all to complete
    high_result = high_prio_handle.get()
    medium_result = medium_prio_handle.get()
    low_result = low_prio_handle.get()

    # Just verify they all completed successfully
    t.assert_equal(high_result, 10)
    t.assert_equal(medium_result, 5)
    t.assert_equal(low_result, 1)

    # Stop the priority worker
    mp.stop_priority_worker()


@runner.test("Advanced - @profiled and metrics")
def test_advanced_profiling(t):
    mp.reset_metrics()

    @mp.profiled
    def profiled_func(n):
        time.sleep(0.05)
        return n * 2

    for i in range(3):
        profiled_func(i)

    metrics = mp.get_metrics("profiled_func")
    t.assert_equal(metrics.total_tasks, 3)
    t.assert_equal(metrics.completed_tasks, 3)

    all_metrics = mp.get_all_metrics()
    t.assert_true("profiled_func" in all_metrics)


# This test is last as it can interfere with other tests
@runner.test("Advanced - Graceful shutdown")
def test_advanced_shutdown(t):
    # Reset shutdown flag for this test
    mp.reset_shutdown()

    @mp.parallel
    def task_for_shutdown():
        time.sleep(1)
        return "done"

    handles = [task_for_shutdown() for _ in range(3)]
    time.sleep(0.1)
    shutdown_success = mp.shutdown(timeout_secs=0.5, cancel_pending=True)

    # Shutdown with 0.5s timeout should timeout (tasks need 1s)
    t.assert_equal(shutdown_success, False)

    # Reset shutdown flag after test
    mp.reset_shutdown()


# =============================================================================
# Run all tests
# =============================================================================
if __name__ == "__main__":
    success = runner.run()
    sys.exit(0 if success else 1)
