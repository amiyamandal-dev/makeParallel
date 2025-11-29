# New Features Documentation

This document describes the new features added to makeParallel:
1. Thread pool size configuration
2. Priority queues
3. Task cancellation improvements
4. Performance profiling tools

## 1. Thread Pool Configuration

Control the size and behavior of the internal thread pool used by parallel operations.

### API

#### `configure_thread_pool(num_threads=None, stack_size=None)`

Configure the global thread pool used by `@parallel_pool` and `parallel_map`.

**Parameters:**
- `num_threads` (int, optional): Number of threads in the pool. Defaults to number of CPU cores.
- `stack_size` (int, optional): Stack size per thread in bytes.

**Example:**
```python
import makeParallel as mp

# Configure thread pool with 8 threads
mp.configure_thread_pool(num_threads=8)

# Configure with custom stack size (2MB)
mp.configure_thread_pool(num_threads=4, stack_size=2*1024*1024)
```

#### `get_thread_pool_info()`

Get information about the current thread pool configuration.

**Returns:** Dictionary with:
- `configured` (bool): Whether a custom pool has been configured
- `current_num_threads` (int): Number of threads currently in the pool

**Example:**
```python
info = mp.get_thread_pool_info()
print(f"Thread pool has {info['current_num_threads']} threads")
```

---

## 2. Priority Queues

Execute tasks based on priority levels, with higher priority tasks executing first.

### API

#### `@parallel_priority` decorator

Decorator for functions that should execute with priority-based scheduling.

**Usage:**
```python
@mp.parallel_priority
def important_task(data):
    return process(data)

# Higher priority number = executes first
handle1 = important_task(data1, priority=10)  # Executes first
handle2 = important_task(data2, priority=5)   # Executes second
handle3 = important_task(data3, priority=1)   # Executes last

results = [h.get() for h in [handle1, handle2, handle3]]
```

#### `start_priority_worker()`

Start the background worker thread that processes the priority queue.

**Note:** This is called automatically when using `@parallel_priority`.

**Example:**
```python
mp.start_priority_worker()
```

#### `stop_priority_worker()`

Stop the priority queue worker thread.

**Example:**
```python
mp.stop_priority_worker()
```

### Priority Queue Example

```python
import makeParallel as mp

@mp.parallel_priority
def batch_process(batch_id):
    # Process batch
    return f"Batch {batch_id} processed"

# Start worker
mp.start_priority_worker()

# Submit tasks with different priorities
urgent = batch_process(1, priority=100)      # Critical
normal = batch_process(2, priority=50)       # Normal
low = batch_process(3, priority=10)          # Low priority

# Tasks execute in priority order: 1, 2, 3
results = [urgent.get(), normal.get(), low.get()]

# Clean up
mp.stop_priority_worker()
```

---

## 3. Task Cancellation Improvements

Enhanced cancellation capabilities with timeouts and better status tracking.

### New AsyncHandle Methods

All `@parallel` decorated functions return an `AsyncHandle` with these new methods:

#### `cancel_with_timeout(timeout_secs)`

Cancel a task with a timeout.

**Parameters:**
- `timeout_secs` (float): Maximum time to wait for cancellation

**Returns:** `bool` - True if cancelled within timeout, False if timeout expired

**Example:**
```python
@mp.parallel
def long_task():
    time.sleep(100)
    return "done"

handle = long_task()

# Try to cancel within 2 seconds
if handle.cancel_with_timeout(2.0):
    print("Successfully cancelled")
else:
    print("Cancellation timed out")
```

#### `is_cancelled()`

Check if the task has been cancelled.

**Returns:** `bool` - True if cancelled, False otherwise

**Example:**
```python
handle = long_task()
handle.cancel()

if handle.is_cancelled():
    print("Task was cancelled")
```

#### `elapsed_time()`

Get the elapsed time since task start.

**Returns:** `float` - Elapsed time in seconds

**Example:**
```python
handle = task()
time.sleep(1)
print(f"Task has been running for {handle.elapsed_time():.2f}s")
```

#### `get_name()`

Get the name of the function being executed.

**Returns:** `str` - Function name

**Example:**
```python
@mp.parallel
def my_function():
    return 42

handle = my_function()
print(f"Running: {handle.get_name()}")  # Output: "Running: my_function"
```

### Complete Cancellation Example

```python
import makeParallel as mp
import time

@mp.parallel
def long_computation(n):
    time.sleep(n)
    return f"Computed for {n}s"

# Start task
handle = long_computation(60)

# Monitor progress
while not handle.is_ready():
    elapsed = handle.elapsed_time()
    print(f"{handle.get_name()} running for {elapsed:.1f}s")

    # Cancel if taking too long
    if elapsed > 5:
        print("Taking too long, cancelling...")
        if handle.cancel_with_timeout(2.0):
            print("Cancelled successfully")
            break

    time.sleep(1)
```

---

## 4. Performance Profiling Tools

Automatic performance tracking and metrics collection.

### API

#### `@profiled` decorator

Decorator that automatically tracks execution time and success/failure rates.

**Example:**
```python
@mp.profiled
def expensive_operation(n):
    return sum(i**2 for i in range(n))

# Function calls are automatically profiled
for i in range(100):
    expensive_operation(10000)

# Check metrics
metrics = mp.get_metrics("expensive_operation")
print(f"Average execution time: {metrics.average_execution_time_ms:.2f}ms")
```

#### `get_metrics(name)`

Get performance metrics for a specific function.

**Parameters:**
- `name` (str): Function name

**Returns:** `PerformanceMetrics` object or `None` if no data

**PerformanceMetrics attributes:**
- `total_tasks` (int): Total number of executions
- `completed_tasks` (int): Number of successful executions
- `failed_tasks` (int): Number of failed executions
- `total_execution_time_ms` (float): Total time spent in milliseconds
- `average_execution_time_ms` (float): Average execution time in milliseconds

**Example:**
```python
metrics = mp.get_metrics("my_function")
if metrics:
    print(f"Executions: {metrics.total_tasks}")
    print(f"Success rate: {metrics.completed_tasks / metrics.total_tasks * 100:.1f}%")
    print(f"Avg time: {metrics.average_execution_time_ms:.2f}ms")
```

#### `get_all_metrics()`

Get metrics for all profiled functions.

**Returns:** Dictionary mapping function names to metric dictionaries

**Example:**
```python
all_metrics = mp.get_all_metrics()

for func_name, metrics in all_metrics.items():
    if not func_name.startswith('_global'):
        print(f"{func_name}:")
        print(f"  Total: {metrics['total_tasks']}")
        print(f"  Avg time: {metrics['average_execution_time_ms']:.2f}ms")

# Global counters
print(f"\nGlobal total: {all_metrics['_global_total']}")
print(f"Global completed: {all_metrics['_global_completed']}")
print(f"Global failed: {all_metrics['_global_failed']}")
```

#### `reset_metrics()`

Clear all collected metrics.

**Example:**
```python
mp.reset_metrics()
```

### Automatic Profiling with @parallel

The `@parallel` decorator automatically profiles tasks! Metrics are collected for all parallel tasks.

**Example:**
```python
import makeParallel as mp

mp.reset_metrics()

@mp.parallel
def parallel_task(n):
    return sum(i**2 for i in range(n))

# Run multiple parallel tasks
handles = [parallel_task(1000000) for _ in range(10)]
results = [h.get() for h in handles]

# Check metrics
metrics = mp.get_all_metrics()
if 'parallel_task' in metrics:
    m = metrics['parallel_task']
    print(f"Ran {m['total_tasks']} tasks")
    print(f"Average time: {m['average_execution_time_ms']:.2f}ms")
    print(f"Total time: {m['total_execution_time_ms']:.2f}ms")
```

### Complete Profiling Example

```python
import makeParallel as mp
import time

# Reset metrics
mp.reset_metrics()

@mp.profiled
def database_query(query_id):
    """Simulated database query"""
    time.sleep(0.1)  # Simulate query time
    if query_id % 10 == 0:
        raise ValueError("Simulated error")
    return f"Result {query_id}"

@mp.profiled
def process_data(data):
    """Data processing"""
    time.sleep(0.05)
    return len(data) * 2

# Run many operations
for i in range(50):
    try:
        database_query(i)
    except ValueError:
        pass  # Expected occasional failures

    process_data(f"data_{i}")

# Analyze performance
all_metrics = mp.get_all_metrics()

print("Performance Report")
print("=" * 60)

for func_name, metrics in all_metrics.items():
    if not func_name.startswith('_global'):
        print(f"\n{func_name}:")
        print(f"  Total executions: {metrics['total_tasks']}")
        print(f"  Successful: {metrics['completed_tasks']}")
        print(f"  Failed: {metrics['failed_tasks']}")
        print(f"  Success rate: {metrics['completed_tasks']/metrics['total_tasks']*100:.1f}%")
        print(f"  Average time: {metrics['average_execution_time_ms']:.2f}ms")
        print(f"  Total time: {metrics['total_execution_time_ms']:.2f}ms")

print(f"\n{'=' * 60}")
print(f"Overall Stats:")
print(f"  Total tasks: {all_metrics['_global_total']}")
print(f"  Completed: {all_metrics['_global_completed']}")
print(f"  Failed: {all_metrics['_global_failed']}")
```

---

## Combined Usage Example

Here's an example using all the new features together:

```python
import makeParallel as mp
import time

# 1. Configure thread pool
mp.configure_thread_pool(num_threads=8)
print(f"Thread pool: {mp.get_thread_pool_info()}")

# 2. Reset metrics for clean slate
mp.reset_metrics()

# 3. Define priority tasks with profiling
@mp.parallel_priority
def critical_task(task_id):
    time.sleep(0.1)
    return f"Critical task {task_id} complete"

@mp.parallel
def normal_task(task_id):
    time.sleep(0.1)
    return f"Normal task {task_id} complete"

# 4. Start priority worker
mp.start_priority_worker()

# 5. Submit mixed priority tasks
priority_handles = [
    critical_task(1, priority=100),
    critical_task(2, priority=50),
    critical_task(3, priority=10),
]

normal_handles = [normal_task(i) for i in range(5)]

# 6. Monitor progress
all_handles = priority_handles + normal_handles

while not all(h.is_ready() for h in all_handles):
    ready = sum(1 for h in all_handles if h.is_ready())
    print(f"Progress: {ready}/{len(all_handles)} tasks complete", end='\r')
    time.sleep(0.1)

print("\nAll tasks complete!")

# 7. Get results
for h in all_handles:
    if not h.is_cancelled():
        print(f"  {h.get()} (took {h.elapsed_time():.2f}s)")

# 8. Check performance metrics
metrics = mp.get_all_metrics()
for name, m in metrics.items():
    if not name.startswith('_global'):
        print(f"\n{name}:")
        print(f"  Tasks: {m['total_tasks']}")
        print(f"  Avg time: {m['average_execution_time_ms']:.2f}ms")

# 9. Cleanup
mp.stop_priority_worker()
```

---

## Migration Guide

### From Old API to New API

#### Thread Pool Configuration
```python
# Before: No configuration possible
# Tasks used default thread pool

# After: Configure as needed
mp.configure_thread_pool(num_threads=16)
```

#### Task Cancellation
```python
# Before: Basic cancellation
handle = task()
handle.cancel()  # Fire and forget

# After: Better control
handle = task()
if handle.cancel_with_timeout(2.0):
    print("Cancelled successfully")

# Check status
if handle.is_cancelled():
    print("Task was cancelled")

# Monitor progress
print(f"Running for {handle.elapsed_time():.2f}s")
```

#### Performance Tracking
```python
# Before: Manual timing
start = time.time()
result = my_function()
duration = time.time() - start
print(f"Took {duration}s")

# After: Automatic profiling
@mp.profiled
def my_function():
    # ... your code ...
    pass

result = my_function()

# Later, check metrics
metrics = mp.get_metrics("my_function")
print(f"Average time: {metrics.average_execution_time_ms}ms")
```

---

## Best Practices

### Thread Pool Configuration
- Set `num_threads` to match your workload
- For CPU-bound tasks: use number of CPU cores
- For I/O-bound tasks: use higher numbers (2-4x cores)
- Configure once at application startup

### Priority Queues
- Use priority values consistently (e.g., 1-100 scale)
- Higher values = higher priority
- Start worker early in your application
- Stop worker at cleanup/shutdown

### Task Cancellation
- Always check `is_cancelled()` before waiting
- Use `cancel_with_timeout()` for graceful shutdown
- Monitor `elapsed_time()` to detect hung tasks
- Use `get_name()` for debugging/logging

### Performance Profiling
- Reset metrics at the start of benchmarks
- Use `@profiled` for functions you want to monitor
- `@parallel` tasks are automatically profiled
- Check metrics periodically in long-running applications
- Reset metrics when starting new test runs

---

## Performance Considerations

### Thread Pool
- Creating a custom pool has one-time overhead
- Configure once at startup
- Larger pools use more memory
- Too many threads can cause contention

### Priority Queue
- Worker thread runs continuously
- Small overhead per task (~10-50μs)
- BinaryHeap operations are O(log n)
- Stop worker when not needed

### Profiling
- Minimal overhead (~1-5μs per task)
- Atomic operations for thread safety
- DashMap for lock-free metrics storage
- Memory grows with number of unique functions

---

## Troubleshooting

### Thread Pool Not Working
```python
# Check configuration
info = mp.get_thread_pool_info()
print(info)

# Reconfigure if needed
mp.configure_thread_pool(num_threads=8)
```

### Priority Tasks Not Executing
```python
# Ensure worker is started
mp.start_priority_worker()

# Check if it's running
# (It starts automatically on first @parallel_priority call)
```

### Metrics Not Appearing
```python
# Ensure function is decorated
@mp.profiled  # Don't forget this!
def my_func():
    pass

# Check if function was called
metrics = mp.get_all_metrics()
print("Tracked functions:", [k for k in metrics.keys() if not k.startswith('_')])
```

### Cancellation Not Working
```python
# Remember: cancellation is cooperative
# The task must have started and checked the cancel flag

handle = task()
time.sleep(0.1)  # Give it time to start
handle.cancel()

# Check status
if handle.is_cancelled():
    print("Successfully cancelled")
```
