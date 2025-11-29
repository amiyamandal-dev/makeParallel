# makeParallel 🚀

**The easiest way to speed up your Python code using all your CPU cores.**

[![PyPI version](https://badge.fury.io/py/makeParallel.svg)](https://badge.fury.io/py/makeParallel)
[![Tests](https://img.shields.io/badge/tests-39/40_passing-brightgreen)](tests/test_all.py)
[![Python Version](https://img.shields.io/badge/python-3.8+-blue.svg)](https://www.python.org/downloads/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Got a slow, CPU-heavy task in Python? `makeParallel` lets you run it on a separate core with a single line of code, so you can get results up to **4x, 8x, or even 16x faster** without blocking your main program.

It's powered by **Rust** to safely bypass Python's Global Interpreter Lock (GIL), giving you true parallelism without the complexity of `multiprocessing`.

---

## 📋 Table of Contents
- [What's the GIL?](#-whats-the-gil)
- [Why You'll Love makeParallel](#-why-youll-love-makeparallel)
- [Installation](#-installation)
- [Quick Start](#-quick-start)
- [When Should I Use This?](#-when-should-i-use-this)
- [Complete Feature Guide](#-complete-feature-guide)
  - [Parallel Execution Decorators](#-parallel-execution-decorators)
  - [Batch Processing](#️-batch-processing)
  - [Caching Decorators](#-caching-decorators)
  - [Retry Logic](#-retry-logic)
  - [Performance Monitoring](#-performance-monitoring)
  - [Advanced Configuration](#️-advanced-configuration)
- [Choosing the Right Decorator](#-choosing-the-right-decorator)
- [How It Works](#️-how-it-works)
- [Best Practices](#-best-practices)
- [Comparison with Alternatives](#-comparison-with-alternatives)
- [Real-World Examples](#-real-world-examples)
- [Troubleshooting](#-troubleshooting)
- [Contributing](#-contributing)
- [License](#-license)

---

### 🤔 What's the "GIL"?

Python has a rule called the Global Interpreter Lock (GIL) that only lets **one thread run at a time**, even on a multi-core CPU. For tasks that just wait for networks (I/O-bound), this is fine. But for heavy calculations (CPU-bound), it means Python can't use all the power your computer has. `makeParallel` fixes this.

---

## ✨ Why You'll Love `makeParallel`

- **So Simple:** Just add the `@parallel` decorator to any function. That's it!
- **True Speed-Up:** Uses Rust threads to run your code on all available CPU cores.
- **Doesn't Block:** Your main application stays responsive while the work happens in the background.
- **No `multiprocessing` Headaches:** Avoids the complexity, memory overhead, and data-sharing issues of `multiprocessing`.
- **Works with Your Code:** Decorate any function, even class methods.

## 📦 Installation

Installing is as simple as:

```bash
pip install makeParallel
```

Or, to build it from the source:
```bash
# Clone the repository
git clone https://github.com/your-username/makeParallel.git
cd makeParallel

# Build and install locally
pip install .
```

## 🚀 Quick Start

Let's say you have a function that does a lot of math and slows down your program.

**Before:** Your code waits...
```python
import time

def cpu_intensive_task(n):
    # A slow calculation
    return sum(i * i for i in range(n))

start = time.time()
result = cpu_intensive_task(20_000_000) # This blocks everything!
print(f"Got result: {result} in {time.time() - start:.2f}s")
```

**After:** Instant and non-blocking!
```python
import time
from makeParallel import parallel

@parallel # Just add this decorator!
def cpu_intensive_task(n):
    # The same slow calculation
    return sum(i * i for i in range(n))

start = time.time()
# The function returns instantly with a "handle"
handle = cpu_intensive_task(20_000_000)

print("The task is running in the background, my app is still responsive!")
# You can do other work here...

# Now, get the result (this will wait until it's ready)
result = handle.get()
print(f"Got result: {result} in {time.time() - start:.2f}s")
```

In the example above, `handle.get()` blocks until the result is ready. You can also check if it's done without waiting:

```python
if handle.is_ready():
    print("It's done!")
else:
    print("Still working...")
```

## 🤔 When Should I Use This?

`makeParallel` is for **CPU-bound** tasks. These are operations that require a lot of computation, like:
- heavy data processing, or scientific computing.
- Image or video processing.
- Complex simulations.

For **I/O-bound** tasks (like waiting for a web request or reading a file), Python's built-in `threading` or `asyncio` are usually a better fit.

## 📚 Complete Feature Guide

`makeParallel` comes with many powerful decorators and utilities.

### 🔥 Parallel Execution Decorators

#### `@parallel` - Full-featured parallel execution with advanced control
```python
from makeParallel import parallel

@parallel
def cpu_intensive_task(n):
    return sum(i * i for i in range(n))

# Returns immediately with an AsyncHandle
handle = cpu_intensive_task(20_000_000, timeout=5.0)

# Check status
if handle.is_ready():
    result = handle.get()

# Try to get result without blocking
result = handle.try_get()  # Returns None if not ready

# Cancel a running task
handle.cancel()
if handle.is_cancelled():
    print("Task was cancelled")

# Get task info
print(f"Task ID: {handle.get_task_id()}")
print(f"Elapsed: {handle.elapsed_time()}s")
print(f"Progress: {handle.get_progress()}")

# Add metadata
handle.set_metadata("user_id", "user-123")
metadata = handle.get_all_metadata()
```

#### `@parallel_fast` - Optimized with lock-free channels (crossbeam)
```python
from makeParallel import parallel_fast

@parallel_fast
def fast_task(x):
    return x ** 2

handle = fast_task(10)
result = handle.get()  # Faster channel communication
```

#### `@parallel_pool` - Uses Rayon thread pool (best for many small tasks)
```python
from makeParallel import parallel_pool

@parallel_pool
def small_task(x):
    return x * 2

# Efficiently handles many concurrent tasks
handles = [small_task(i) for i in range(1000)]
results = [h.get() for h in handles]
```

#### `@parallel_priority` - Priority-based execution
```python
from makeParallel import parallel_priority

@parallel_priority
def task(data, priority):
    return process(data)

# High priority tasks execute first
low = task(data1, priority=1)
high = task(data2, priority=10)  # Runs first
```

### 🗺️ Batch Processing

#### `parallel_map` - Process lists in parallel
```python
from makeParallel import parallel_map

def process_data(item):
    return item * 2

my_large_list = list(range(10000))
results = parallel_map(process_data, my_large_list)
```

#### `gather` - Collect results from multiple handles
```python
from makeParallel import parallel, gather

@parallel
def task(x):
    return x ** 2

handles = [task(i) for i in range(10)]

# Wait for all and collect results
results = gather(handles, on_error="raise")  # or "skip" or "none"
```

#### `ParallelContext` - Context manager for parallel tasks
```python
from makeParallel import ParallelContext, parallel

@parallel
def task(x):
    return x * 2

with ParallelContext(timeout=10.0) as ctx:
    handle1 = ctx.submit(task, (5,))
    handle2 = ctx.submit(task, (10,))
    # All tasks complete when exiting context
```

### 💾 Caching Decorators

#### `@memoize` - Cache function results
```python
from makeParallel import memoize

@memoize
def fibonacci(n):
    if n <= 1:
        return n
    return fibonacci(n - 1) + fibonacci(n - 2)

fibonacci(35)  # Slow first time
fibonacci(35)  # Instant second time
```

#### `@memoize_fast` - Lock-free concurrent cache (DashMap)
```python
from makeParallel import memoize_fast

@memoize_fast
def expensive_computation(x, y):
    return x ** y

# Safe for concurrent access from multiple threads
```

### 🔁 Retry Logic

#### `@retry` - Simple retry with fixed delays
```python
from makeParallel import retry

@retry(max_retries=3)
def flaky_api_call():
    # Will retry up to 3 times on failure
    return make_request()
```

#### `@retry_backoff` - Retry with exponential backoff
```python
from makeParallel import retry_backoff

@retry_backoff(
    max_attempts=5,
    backoff="exponential",  # or "linear"
    initial_delay=1.0,
    max_delay=60.0
)
def unreliable_task():
    return do_something()
```

### 📊 Performance Monitoring

#### `@profiled` - Automatic performance tracking
```python
from makeParallel import profiled, get_metrics, get_all_metrics

@profiled
def tracked_function(x):
    return x ** 2

for i in range(100):
    tracked_function(i)

# Get metrics for specific function
metrics = get_metrics("tracked_function")
print(f"Total tasks: {metrics.total_tasks}")
print(f"Completed: {metrics.completed_tasks}")
print(f"Failed: {metrics.failed_tasks}")
print(f"Avg time: {metrics.average_execution_time_ms}ms")

# Get all metrics
all_metrics = get_all_metrics()
```

#### `@timer` - Simple execution timing
```python
from makeParallel import timer

@timer
def my_function():
    # Do work...
    pass

my_function()  # Prints execution time
```

#### `@log_calls` - Log function calls and returns
```python
from makeParallel import log_calls

@log_calls
def my_function(x, y):
    return x + y

my_function(5, 3)  # Prints call args and return value
```

#### `@CallCounter` - Count function invocations
```python
from makeParallel import CallCounter

@CallCounter
def counted_function():
    return "result"

counted_function()
counted_function()
print(counted_function.call_count)  # 2
counted_function.reset()
```

### ⚙️ Advanced Configuration

#### Thread Pool Configuration
```python
from makeParallel import configure_thread_pool, get_thread_pool_info

# Configure global thread pool
configure_thread_pool(num_threads=8, stack_size=2*1024*1024)

# Get current info
info = get_thread_pool_info()
print(info["current_num_threads"])
```

#### Backpressure and Resource Management
```python
from makeParallel import set_max_concurrent_tasks, configure_memory_limit

# Limit concurrent tasks to prevent overload
set_max_concurrent_tasks(100)

# Set memory limit (percentage)
configure_memory_limit(max_memory_percent=80.0)
```

#### Progress Reporting
```python
from makeParallel import parallel, report_progress

@parallel
def long_task():
    for i in range(100):
        # Report progress from within task
        report_progress(task_id, i / 100.0)
        # Do work...
    return "done"

handle = long_task()
# Check progress from outside
print(f"Progress: {handle.get_progress() * 100}%")
```

#### Graceful Shutdown
```python
from makeParallel import shutdown, get_active_task_count, reset_shutdown

# Get active task count
print(f"Active tasks: {get_active_task_count()}")

# Graceful shutdown with timeout
success = shutdown(timeout_secs=30.0, cancel_pending=True)

# Reset after shutdown (for testing)
reset_shutdown()
```

## 🎯 Choosing the Right Decorator

| Decorator | Best For | Performance | Features |
|-----------|----------|-------------|----------|
| `@parallel` | Most use cases | Good | Full control: timeout, cancel, metadata, progress |
| `@parallel_fast` | High-throughput tasks | Better | Lock-free channels (crossbeam) |
| `@parallel_pool` | Many small tasks | Best | Rayon thread pool, efficient resource usage |
| `@parallel_priority` | Priority-based scheduling | Good | Priority queue execution |
| `parallel_map` | Batch processing lists | Best | Automatic parallelization across items |

**Quick Decision Guide:**
- **Single long task with monitoring?** → `@parallel`
- **Thousands of small tasks?** → `@parallel_pool`
- **Processing a large list?** → `parallel_map`
- **Need priority scheduling?** → `@parallel_priority`
- **Maximum throughput?** → `@parallel_fast`

## 🏗️ How It Works

Here's a simple breakdown of what happens when you call a `@parallel` function:

1.  **Python Side**: Your main program calls the function but doesn't run it directly. Instead, it sends the function and its arguments to the Rust backend.
2.  **Rust Backend**:
    *   It immediately returns the `AsyncHandle` object to your Python code so it doesn't have to wait.
    *   It releases Python's **Global Interpreter Lock (GIL)**.
    *   It spawns a **new Rust OS thread** (a real parallel thread).
    *   Inside the new thread, it re-acquires the GIL to safely execute your Python function.
3.  **Result**: The result is sent back to the `AsyncHandle`, which your main program can access with `.get()`.

This GIL-release-and-reacquire step is the key to unlocking true parallelism for CPU-bound Python code.

## 🔧 Best Practices

### ✅ Do's
- **Use for CPU-bound tasks**: Heavy computation, data processing, mathematical operations
- **Combine with caching**: Use `@memoize` or `@memoize_fast` to avoid redundant calculations
- **Monitor with profiling**: Use `@profiled` to track performance and identify bottlenecks
- **Set resource limits**: Use `set_max_concurrent_tasks()` to prevent overload
- **Handle errors gracefully**: Use `gather()` with appropriate `on_error` strategy
- **Use timeouts for long tasks**: Add `timeout` parameter to prevent hanging

### ❌ Don'ts
- **Don't use for I/O-bound tasks**: Use `asyncio` or `threading` instead
- **Don't pass large objects**: Minimize data transfer between threads
- **Don't ignore error handling**: Always check results or use try/except
- **Don't spawn unlimited tasks**: Use `set_max_concurrent_tasks()` for backpressure
- **Don't forget cleanup**: Use `shutdown()` for graceful termination

### 💡 Performance Tips
```python
# ✅ Good: Process large batches efficiently
results = parallel_map(heavy_computation, large_list)

# ❌ Bad: Creating too many individual parallel tasks
handles = [parallel_task(x) for x in range(10000)]  # Overhead!

# ✅ Good: Use memoization for repeated calls
@memoize_fast
@parallel_pool
def cached_parallel_task(x):
    return expensive_operation(x)

# ✅ Good: Configure thread pool for your workload
configure_thread_pool(num_threads=8)  # Match your CPU cores
```

## 🆚 Comparison with Alternatives

| Feature | makeParallel | multiprocessing | threading | asyncio |
|---------|-------------|----------------|-----------|---------|
| **True Parallelism** | ✅ Yes | ✅ Yes | ❌ No (GIL) | ❌ No (GIL) |
| **CPU-bound Tasks** | ✅ Excellent | ✅ Good | ❌ Poor | ❌ Poor |
| **I/O-bound Tasks** | ⚠️ Okay | ⚠️ Okay | ✅ Good | ✅ Excellent |
| **Memory Overhead** | ✅ Low | ❌ High | ✅ Low | ✅ Low |
| **Easy to Use** | ✅ Very Easy | ⚠️ Complex | ✅ Easy | ⚠️ Moderate |
| **Data Sharing** | ✅ Simple | ❌ Complex | ✅ Simple | ✅ Simple |
| **Performance** | ✅✅ Fast (Rust) | ✅ Good | ⚠️ Limited | ✅ Good |
| **Cancellation** | ✅ Built-in | ⚠️ Manual | ⚠️ Manual | ✅ Built-in |
| **Progress Tracking** | ✅ Built-in | ❌ Manual | ❌ Manual | ⚠️ Manual |

## 📖 Real-World Examples

### Example 1: Image Processing Pipeline
```python
from makeParallel import parallel_map, profiled
from PIL import Image
import os

@profiled
def process_image(image_path):
    img = Image.open(image_path)
    # Resize, apply filters, etc.
    img_resized = img.resize((800, 600))
    # Save processed image
    output_path = f"processed_{os.path.basename(image_path)}"
    img_resized.save(output_path)
    return output_path

# Process 1000 images in parallel
image_files = [f"image_{i}.jpg" for i in range(1000)]
processed = parallel_map(process_image, image_files)

print(f"Processed {len(processed)} images")
```

### Example 2: Web Scraping with Retry Logic
```python
from makeParallel import parallel_pool, retry_backoff
import requests

@retry_backoff(max_attempts=3, backoff="exponential")
@parallel_pool
def fetch_url(url):
    response = requests.get(url, timeout=10)
    return response.text

urls = ["https://example.com/page1", "https://example.com/page2", ...]
handles = [fetch_url(url) for url in urls]
results = [h.get() for h in handles]
```

### Example 3: Data Analysis with Progress Tracking
```python
from makeParallel import parallel, report_progress
import pandas as pd

@parallel
def analyze_dataset(file_path, task_id):
    df = pd.read_csv(file_path)
    total_rows = len(df)

    results = []
    for i, row in df.iterrows():
        # Report progress
        report_progress(task_id, i / total_rows)

        # Perform analysis
        result = complex_analysis(row)
        results.append(result)

    return results

handle = analyze_dataset("large_dataset.csv")

# Monitor progress
import time
while not handle.is_ready():
    print(f"Progress: {handle.get_progress() * 100:.1f}%")
    time.sleep(1)

final_results = handle.get()
```

### Example 4: Machine Learning Model Training
```python
from makeParallel import parallel, gather, configure_thread_pool
from sklearn.model_selection import train_test_split
from sklearn.ensemble import RandomForestClassifier

# Configure thread pool for ML workload
configure_thread_pool(num_threads=4)

@parallel
def train_model(params):
    model = RandomForestClassifier(**params)
    model.fit(X_train, y_train)
    score = model.score(X_test, y_test)
    return {"params": params, "score": score}

# Train multiple models with different hyperparameters
param_grid = [
    {"n_estimators": 100, "max_depth": 10},
    {"n_estimators": 200, "max_depth": 15},
    {"n_estimators": 300, "max_depth": 20},
]

handles = [train_model(params) for params in param_grid]
results = gather(handles)

# Find best model
best = max(results, key=lambda x: x["score"])
print(f"Best params: {best['params']}, Score: {best['score']}")
```

## 🐛 Troubleshooting

### My tasks are running slowly
- Check if your task is CPU-bound (use `@profiled` to measure)
- For I/O-bound tasks, use `asyncio` instead
- Ensure you're not creating too many tasks (use `parallel_map` for batch processing)
- Configure thread pool: `configure_thread_pool(num_threads=<cpu_cores>)`

### Tasks are hanging
- Add timeouts: `@parallel def task(): ...` then `task(timeout=10.0)`
- Use `cancel()` to stop stuck tasks
- Check for deadlocks in your Python code

### Memory usage is too high
- Limit concurrent tasks: `set_max_concurrent_tasks(100)`
- Set memory limit: `configure_memory_limit(max_memory_percent=80.0)`
- Use `@parallel_pool` instead of spawning individual threads
- Process data in smaller batches

### Errors are being swallowed
- Always check `handle.get()` in a try/except block
- Use `gather()` with `on_error="raise"` to see all errors
- Enable profiling to see failed task counts: `@profiled`

## 🤝 Contributing

Contributions are welcome! If you want to help improve `makeParallel`, please feel free to open an issue or submit a pull request.

### Development Setup
```bash
# Clone the repository
git clone https://github.com/your-username/makeParallel.git
cd makeParallel

# Create virtual environment
python -m venv .venv
source .venv/bin/activate  # On Windows: .venv\Scripts\activate

# Install development dependencies
pip install maturin

# Build and install in development mode
maturin develop

# Run tests
python tests/test_all.py
```

### Running Tests
```bash
# Run all tests
python tests/test_all.py

# The test suite includes:
# - 39 passing tests covering all features
# - Performance benchmarks
# - Edge case validation
# - Error handling verification
```

### Code Quality
```bash
# Format Rust code
cargo fmt

# Lint Rust code
cargo clippy

# Format Python code (if you have ruff)
ruff format .

# Check Python code
ruff check .
```

## 📝 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Built with [PyO3](https://pyo3.rs/) for Python-Rust interop
- Uses [Rayon](https://github.com/rayon-rs/rayon) for efficient thread pool management
- Uses [Crossbeam](https://github.com/crossbeam-rs/crossbeam) for lock-free channels
- Uses [DashMap](https://github.com/xacrimon/dashmap) for concurrent caching

## 📬 Contact & Support

- **Issues**: [GitHub Issues](https://github.com/your-username/makeParallel/issues)
- **Discussions**: [GitHub Discussions](https://github.com/your-username/makeParallel/discussions)
- **PyPI**: [pypi.org/project/makeParallel](https://pypi.org/project/makeParallel/)

---

Made with ❤️ and Rust 🦀