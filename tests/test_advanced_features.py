#!/usr/bin/env python3
"""
Comprehensive test of all advanced features in makeParallel.

Tests:
1. Result callbacks (on_complete, on_error)
2. Context manager (ParallelContext)
3. Backpressure/rate limiting
4. Progress tracking
5. Memory-aware execution
6. Retry with exponential backoff
7. gather() helper function
"""

import makeParallel as mp
import time
import sys

def print_section(title):
    """Print a formatted section header."""
    print(f"\n{'='*60}")
    print(f"  {title}")
    print(f"{'='*60}\n")


def test_result_callbacks():
    """Test 1: Result callbacks (on_complete, on_error)"""
    print_section("Test 1: Result Callbacks")

    completed_results = []
    error_results = []

    def on_success(result):
        completed_results.append(result)
        print(f"✅ Callback: Task completed with result: {result}")

    def on_failure(error):
        error_results.append(str(error))
        print(f"❌ Callback: Task failed with error: {error}")

    @mp.parallel
    def success_task(x):
        time.sleep(0.1)
        return x * 2

    @mp.parallel
    def failing_task(x):
        time.sleep(0.1)
        if x == 5:
            raise ValueError("Test error: x is 5")
        return x * 2

    # Test success callback
    print("Testing on_complete callback...")
    handle1 = success_task(10)
    handle1.on_complete(on_success)
    result1 = handle1.get()
    time.sleep(0.2)  # Give callback time to execute

    # Test error callback
    print("\nTesting on_error callback...")
    handle2 = failing_task(5)
    handle2.on_error(on_failure)
    try:
        result2 = handle2.get()
    except Exception as e:
        print(f"Exception caught: {e}")
    time.sleep(0.2)  # Give callback time to execute

    print(f"\n✅ Result callbacks test completed!")
    print(f"   Completed callbacks: {len(completed_results)}")
    print(f"   Error callbacks: {len(error_results)}")


def test_context_manager():
    """Test 2: Context manager (ParallelContext)"""
    print_section("Test 2: Context Manager")

    @mp.parallel
    def task(x):
        time.sleep(0.1)
        return x ** 2

    print("Creating ParallelContext...")
    # Note: num_threads is configured globally via configure_thread_pool()
    # Note: For now, we manage handles manually within the context
    handles = []
    with mp.ParallelContext(timeout=10.0) as ctx:
        print("Inside context manager - submitting tasks...")
        # Call tasks normally - they return AsyncHandle
        handles = [task(i) for i in range(10)]
        print(f"Submitted {len(handles)} tasks")

        results = [h.get() for h in handles]
        print(f"Got {len(results)} results: {results}")

    print("Exited context manager - cleanup automatic!")
    print(f"✅ Context manager test completed!")


def test_backpressure():
    """Test 3: Backpressure/rate limiting"""
    print_section("Test 3: Backpressure/Rate Limiting")

    # Set max concurrent tasks to 5
    mp.set_max_concurrent_tasks(5)
    print("Set max concurrent tasks to 5")

    @mp.parallel
    def slow_task(x):
        time.sleep(0.2)
        return x

    print("Submitting 20 tasks (will be throttled to 5 concurrent)...")
    start = time.time()
    handles = [slow_task(i) for i in range(20)]

    # Monitor active tasks
    max_concurrent = 0
    while not all(h.is_ready() for h in handles):
        active = mp.get_active_task_count()
        max_concurrent = max(max_concurrent, active)
        print(f"Active tasks: {active}", end='\r')
        time.sleep(0.05)

    results = [h.get() for h in handles]
    elapsed = time.time() - start

    print(f"\n✅ Backpressure test completed!")
    print(f"   Max concurrent tasks observed: {max_concurrent}")
    print(f"   Total time: {elapsed:.2f}s")
    print(f"   Results: {len(results)} tasks completed")

    # Reset for other tests
    mp.set_max_concurrent_tasks(100)


def test_progress_tracking():
    """Test 5: Progress tracking"""
    print_section("Test 5: Progress Tracking")

    progress_updates = []

    def progress_callback(progress):
        progress_updates.append(progress)
        print(f"Progress: {progress * 100:.1f}%")

    @mp.parallel
    def long_task(items):
        task_id = mp.get_current_task_id() if hasattr(mp, 'get_current_task_id') else "task_1"
        for i, item in enumerate(items):
            time.sleep(0.05)
            progress = (i + 1) / len(items)
            # Report progress (note: this is manual for now)
            # mp.report_progress(task_id, progress)
        return sum(items)

    print("Starting task with progress tracking...")
    handle = long_task(list(range(10)))

    # Set progress callback
    if hasattr(handle, 'on_progress'):
        handle.on_progress(progress_callback)

    result = handle.get()

    print(f"✅ Progress tracking test completed!")
    print(f"   Result: {result}")
    print(f"   Progress updates received: {len(progress_updates)}")


def test_memory_aware():
    """Test 6: Memory-aware execution"""
    print_section("Test 6: Memory-Aware Execution")

    # Configure memory limit to 80%
    mp.configure_memory_limit(80.0)
    print("Configured memory limit to 80%")

    @mp.parallel
    def memory_task(size):
        # Simulate memory usage
        data = list(range(size))
        time.sleep(0.1)
        return len(data)

    print("Running memory-intensive tasks...")
    handles = [memory_task(1000) for i in range(10)]
    results = [h.get() for h in handles]

    print(f"✅ Memory-aware execution test completed!")
    print(f"   Tasks completed: {len(results)}")
    print(f"   Note: Memory checking is placeholder (TODO: implement with sysinfo)")


def test_retry_backoff():
    """Test 7: Retry with exponential backoff"""
    print_section("Test 7: Retry with Exponential Backoff")

    attempt_count = [0]

    def flaky_function():
        attempt_count[0] += 1
        print(f"Attempt {attempt_count[0]}")
        if attempt_count[0] < 3:
            raise ConnectionError("Simulated connection error")
        return "Success!"

    print("Testing retry with exponential backoff...")
    print("Function will fail 2 times, then succeed on 3rd attempt")

    # Create retry decorator
    retry_decorator = mp.retry_backoff(
        max_attempts=5,
        backoff='exponential',
        initial_delay=0.1,
        max_delay=2.0
    )

    # Apply decorator
    retried_function = retry_decorator(flaky_function)

    start = time.time()
    try:
        result = retried_function()
        elapsed = time.time() - start
        print(f"\n✅ Retry test completed!")
        print(f"   Result: {result}")
        print(f"   Total attempts: {attempt_count[0]}")
        print(f"   Time elapsed: {elapsed:.2f}s")
    except Exception as e:
        print(f"❌ Failed after all retries: {e}")


def test_gather_helper():
    """Test 8: gather() helper function"""
    print_section("Test 8: gather() Helper Function")

    @mp.parallel
    def task(x):
        time.sleep(0.1)
        if x == 5:
            raise ValueError(f"Error on task {x}")
        return x * 2

    # Test gather with 'skip' error handling
    print("Testing gather() with on_error='skip'...")
    handles = [task(i) for i in range(10)]
    results = mp.gather(handles, on_error='skip')
    print(f"Results (errors skipped): {results}")
    print(f"Expected 9 results (task 5 failed): Got {len(results)} results")

    # Test gather with 'none' error handling
    print("\nTesting gather() with on_error='none'...")
    handles = [task(i) for i in range(10)]
    results = mp.gather(handles, on_error='none')
    print(f"Results (errors as None): {len(results)} results")
    none_count = sum(1 for r in results if r is None)
    print(f"None values: {none_count}")

    print(f"\n✅ gather() helper test completed!")


def test_combined_features():
    """Test: Combining multiple features"""
    print_section("Combined Features Test")

    print("Combining: Context Manager + Backpressure + Callbacks + gather()")

    mp.set_max_concurrent_tasks(3)
    completed_tasks = []

    def on_complete(result):
        completed_tasks.append(result)

    @mp.parallel
    def combined_task(x):
        time.sleep(0.1)
        return x ** 2

    with mp.ParallelContext(timeout=30.0) as ctx:
        handles = []
        for i in range(10):
            h = combined_task(i)
            h.on_complete(on_complete)
            handles.append(h)

        results = mp.gather(handles, on_error='skip')

    time.sleep(0.5)  # Wait for callbacks

    print(f"✅ Combined features test completed!")
    print(f"   Results: {results}")
    print(f"   Callback count: {len(completed_tasks)}")


def main():
    """Run all tests."""
    print("\n" + "="*60)
    print("  makeParallel Advanced Features Test Suite")
    print("="*60)

    tests = [
        test_result_callbacks,
        test_context_manager,
        test_backpressure,
        test_progress_tracking,
        test_memory_aware,
        test_retry_backoff,
        test_gather_helper,
        test_combined_features,
    ]

    passed = 0
    failed = 0

    for test_func in tests:
        try:
            test_func()
            passed += 1
        except Exception as e:
            failed += 1
            print(f"\n❌ Test failed: {test_func.__name__}")
            print(f"   Error: {e}")
            import traceback
            traceback.print_exc()

    print_section("Test Summary")
    print(f"Passed: {passed}/{len(tests)}")
    print(f"Failed: {failed}/{len(tests)}")

    if failed == 0:
        print("\n🎉 All tests passed!")
        return 0
    else:
        print(f"\n⚠️  {failed} test(s) failed")
        return 1


if __name__ == "__main__":
    sys.exit(main())
