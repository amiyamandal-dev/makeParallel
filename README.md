# makeParallel 🚀

**The easiest way to speed up your Python code using all your CPU cores.**

[![PyPI version](https://badge.fury.io/py/makeParallel.svg)](https://badge.fury.io/py/makeParallel)
[![Tests](https://img.shields.io/badge/tests-passing-brightgreen)](tests/test_all.py)
[![Python Version](https://img.shields.io/badge/python-3.8+-blue.svg)](https://www.python.org/downloads/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Got a slow, CPU-heavy task in Python? `makeParallel` lets you run it on a separate core with a single line of code, so you can get results up to **4x, 8x, or even 16x faster** without blocking your main program.

It's powered by Rust to safely bypass Python's Global Interpreter Lock (GIL), giving you true parallelism without the complexity of `multiprocessing`.

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

## 📚 More Examples

`makeParallel` comes with other useful decorators.

### `parallel_map`: Process a list in parallel

The fastest way to apply a function to every item in a list. It automatically distributes the work across all your CPU cores.

```python
from makeParallel import parallel_map

def process_data(item):
    return item * 2 # Some CPU-intensive work

my_large_list = list(range(1000))

# parallel_map will run process_data on the list items in parallel
results = parallel_map(process_data, my_large_list)
```

### `@timer`: Measure how long a function takes

```python
from makeParallel import timer
import time

@timer
def my_function():
    time.sleep(1)

my_function() # Prints: 'my_function' executed in 1.00 seconds
```

### `@memoize`: Cache results to avoid re-calculating

A smart decorator that caches the return value of a function. The next time you call it with the same arguments, you get the result instantly.

```python
from makeParallel import memoize

@memoize
def fibonacci(n):
    if n <= 1:
        return n
    return fibonacci(n - 1) + fibonacci(n - 2)

# The first call is slow...
fibonacci(35)
# The second call is instantaneous!
fibonacci(35)
```

## 🏗️ How It Works

Here’s a simple breakdown of what happens when you call a `@parallel` function:

1.  **Python Side**: Your main program calls the function but doesn't run it directly. Instead, it sends the function and its arguments to the Rust backend.
2.  **Rust Backend**:
    *   It immediately returns the `AsyncHandle` object to your Python code so it doesn't have to wait.
    *   It releases Python's **Global Interpreter Lock (GIL)**.
    *   It spawns a **new Rust OS thread** (a real parallel thread).
    *   Inside the new thread, it re-acquires the GIL to safely execute your Python function.
3.  **Result**: The result is sent back to the `AsyncHandle`, which your main program can access with `.get()`.

This GIL-release-and-reacquire step is the key to unlocking true parallelism for CPU-bound Python code.

## 🤝 Contributing

Contributions are welcome! If you want to help improve `makeParallel`, please feel free to open an issue or submit a pull request.

1.  Clone the repo.
2.  Install the project with `pip install -e .[dev]`.
3.  Run tests with `pytest`.
4.  Check formatting with `cargo fmt` and `ruff format .`.
5.  Run lints with `cargo clippy` and `ruff check .`.

## 📝 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.