# Callbacks and Task Dependencies - User Guide

## Overview

This document describes the new callback system and task dependency features added to `makeparallel`.

## New Features

### 1. **Callbacks** - React to task events
- `on_complete` - Called when task finishes successfully
- `on_error` - Called when task fails
- `on_progress` - Called when task reports progress

### 2. **Task Dependencies** - Chain tasks together
- `@parallel_with_deps` decorator
- Tasks wait for dependencies before executing
- Dependency results passed as arguments

---

## Callbacks

### on_complete Callback

Called when a task completes successfully with the result.

**Example**:
```python
import makeparallel as mp

@mp.parallel
def process_data(data):
    # Do some work
    return f"Processed {len(data)} items"

handle = process_data([1, 2, 3])

# Set completion callback
handle.on_complete(lambda result: print(f"Done: {result}"))

result = handle.get()
# Output: Done: Processed 3 items
```

**Use Cases**:
- Logging task completion
- Triggering next steps
- Sending notifications
- Updating UI

---

### on_error Callback

Called when a task fails with the error message.

**Example**:
```python
@mp.parallel
def risky_operation():
    raise ValueError("Something went wrong!")

handle = risky_operation()

# Set error callback
handle.on_error(lambda error: print(f"Error occurred: {error}"))

try:
    handle.get()
except Exception as e:
    print(f"Caught: {e}")
# Output: Error occurred: [error message]
```

**Use Cases**:
- Error logging
- Alerting/monitoring
- Fallback actions
- Error recovery

---

### on_progress Callback

Called whenever the task reports progress using `report_progress()`.

**Example**:
```python
@mp.parallel
def download_file(url):
    chunks = 100
    for i in range(chunks):
        # Download chunk
        download_chunk(url, i)

        # Report progress
        mp.report_progress((i + 1) / chunks)

    return "Download complete"

handle = download_file("https://example.com/file.zip")

# Set progress callback
handle.on_progress(lambda p: print(f"Progress: {p*100:.1f}%"))

result = handle.get()
# Output:
# Progress: 1.0%
# Progress: 2.0%
# ...
# Progress: 100.0%
```

**Use Cases**:
- Progress bars
- Real-time status updates
- Performance monitoring
- User feedback

---

### Combining All Callbacks

```python
import makeparallel as mp

@mp.parallel
def comprehensive_task(n):
    try:
        for i in range(n):
            # Do work
            process_item(i)

            # Report progress
            mp.report_progress((i + 1) / n)

        return f"Processed {n} items"
    except Exception as e:
        raise RuntimeError(f"Failed at item {i}: {e}")

handle = comprehensive_task(10)

# Set all callbacks
handle.on_progress(lambda p: update_progress_bar(p))
handle.on_complete(lambda r: log_success(r))
handle.on_error(lambda e: send_alert(e))

result = handle.get()
```

---

## Task Dependencies

### Basic Dependency

Tasks can depend on other tasks. Dependent tasks wait for their dependencies to complete before starting.

**Example**:
```python
import makeparallel as mp

@mp.parallel_with_deps
def fetch_data():
    # Fetch data from API
    return {"user": "Alice", "age": 30}

@mp.parallel_with_deps
def process_data(deps):
    # deps is a tuple of dependency results
    data = deps[0]
    return f"Processed {data['user']}, age {data['age']}"

# Start first task
handle1 = fetch_data()

# Start second task that depends on first
handle2 = process_data(depends_on=[handle1])

result = handle2.get()
# Output: Processed Alice, age 30
```

**How it works**:
1. `fetch_data()` starts immediately
2. `process_data()` waits for `fetch_data()` to complete
3. Result from `fetch_data()` is passed as first argument (`deps`) to `process_data()`
4. `process_data()` executes with the dependency result

---

### Multiple Dependencies

A task can depend on multiple other tasks.

**Example**:
```python
@mp.parallel_with_deps
def fetch_user():
    return {"name": "Alice", "id": 123}

@mp.parallel_with_deps
def fetch_orders():
    return [{"id": 1, "item": "Book"}, {"id": 2, "item": "Pen"}]

@mp.parallel_with_deps
def generate_report(deps):
    user_data, orders_data = deps
    return f"Report for {user_data['name']}: {len(orders_data)} orders"

h_user = fetch_user()
h_orders = fetch_orders()

# Task depends on both
h_report = generate_report(depends_on=[h_user, h_orders])

report = h_report.get()
# Output: Report for Alice: 2 orders
```

---

### Dependency Chains

Create chains of dependent tasks.

**Example**:
```python
@mp.parallel_with_deps
def step1():
    return 10

@mp.parallel_with_deps
def step2(deps):
    return deps[0] * 2  # 20

@mp.parallel_with_deps
def step3(deps):
    return deps[0] + 5  # 25

@mp.parallel_with_deps
def step4(deps):
    return deps[0] ** 2  # 625

h1 = step1()
h2 = step2(depends_on=[h1])
h3 = step3(depends_on=[h2])
h4 = step4(depends_on=[h3])

final_result = h4.get()
# Output: 625
```

---

### Complex Dependency Patterns

#### Diamond Pattern

```python
@mp.parallel_with_deps
def source():
    return "data"

@mp.parallel_with_deps
def branch_a(deps):
    return f"A({deps[0]})"

@mp.parallel_with_deps
def branch_b(deps):
    return f"B({deps[0]})"

@mp.parallel_with_deps
def merge(deps):
    return f"Merged: {deps[0]} + {deps[1]}"

h_source = source()
h_a = branch_a(depends_on=[h_source])
h_b = branch_b(depends_on=[h_source])
h_merge = merge(depends_on=[h_a, h_b])

result = h_merge.get()
# Output: Merged: A(data) + B(data)
```

#### Fan-out / Fan-in Pattern

```python
@mp.parallel_with_deps
def split_work():
    return [1, 2, 3, 4, 5]

@mp.parallel_with_deps
def worker1(deps):
    return sum(deps[0][:2])  # 3

@mp.parallel_with_deps
def worker2(deps):
    return sum(deps[0][2:4])  # 7

@mp.parallel_with_deps
def worker3(deps):
    return sum(deps[0][4:])  # 5

@mp.parallel_with_deps
def combine(deps):
    return sum(deps)  # 15

h_split = split_work()
h_w1 = worker1(depends_on=[h_split])
h_w2 = worker2(depends_on=[h_split])
h_w3 = worker3(depends_on=[h_split])
h_combine = combine(depends_on=[h_w1, h_w2, h_w3])

total = h_combine.get()
# Output: 15
```

---

### Dependencies with Callbacks

Combine dependencies and callbacks for powerful workflows.

**Example**:
```python
progress_tracker = {}

@mp.parallel_with_deps
def long_running_task(task_id):
    for i in range(10):
        time.sleep(0.1)
        mp.report_progress((i + 1) / 10)
    return f"Task {task_id} complete"

@mp.parallel_with_deps
def aggregate_results(deps):
    return f"All tasks done: {len(deps)} results"

# Start multiple tasks
handles = []
for i in range(3):
    h = long_running_task(i)

    # Set progress callback for each
    h.on_progress(lambda p, tid=i: progress_tracker.update({tid: p}))

    handles.append(h)

# Aggregate all results
h_final = aggregate_results(depends_on=handles)

result = h_final.get()
print(f"Final: {result}")
print(f"Progress tracking: {progress_tracker}")
```

---

## API Reference

### AsyncHandle Methods

#### `handle.on_complete(callback)`
Register a callback for task completion.

**Parameters**:
- `callback` (callable): Function taking one argument (the result)

**Returns**: None

**Example**:
```python
handle.on_complete(lambda r: print(f"Done: {r}"))
```

---

#### `handle.on_error(callback)`
Register a callback for task errors.

**Parameters**:
- `callback` (callable): Function taking one argument (error message string)

**Returns**: None

**Example**:
```python
handle.on_error(lambda e: log_error(e))
```

---

#### `handle.on_progress(callback)`
Register a callback for progress updates.

**Parameters**:
- `callback` (callable): Function taking one argument (progress 0.0-1.0)

**Returns**: None

**Example**:
```python
handle.on_progress(lambda p: update_ui(p * 100))
```

---

### Decorators

#### `@parallel_with_deps`
Decorator for functions that support task dependencies.

**Usage**:
```python
@mp.parallel_with_deps
def my_task(deps, arg1, arg2):
    # deps is tuple of dependency results
    # arg1, arg2 are regular arguments
    pass

h = my_task(arg1=..., arg2=..., depends_on=[h1, h2])
```

**Parameters**:
- Function parameters (excluding `deps`)
- `depends_on` (optional): List of AsyncHandle objects to depend on
- `timeout` (optional): Timeout in seconds

**Returns**: AsyncHandle

---

## Best Practices

### 1. **Callback Error Handling**
Always handle errors in callbacks:

```python
def safe_callback(result):
    try:
        process_result(result)
    except Exception as e:
        log_error(f"Callback failed: {e}")

handle.on_complete(safe_callback)
```

### 2. **Dependency Limits**
Don't create too many dependency levels:

```python
# ❌ Bad: Deep nesting (hard to debug)
h1 = task1()
h2 = task2(depends_on=[h1])
h3 = task3(depends_on=[h2])
h4 = task4(depends_on=[h3])
# ... 20 more levels

# ✓ Good: Keep it shallow
h1 = task1()
h2 = task2()
h3 = combine(depends_on=[h1, h2])
```

### 3. **Progress Reporting**
Report progress at meaningful intervals:

```python
@mp.parallel
def process_items(items):
    total = len(items)
    for i, item in enumerate(items):
        process(item)

        # Report every 10% or at least every 10 items
        if i % max(1, total // 10) == 0:
            mp.report_progress(i / total)

    mp.report_progress(1.0)  # Always report 100%
```

### 4. **Dependency Timeouts**
Set timeouts for tasks with dependencies:

```python
h1 = long_task()
h2 = dependent_task(depends_on=[h1], timeout=60.0)  # 60 second timeout
```

---

## Troubleshooting

### Callbacks Not Firing
- Ensure you call `handle.get()` or `handle.wait()`
- Callbacks fire when result is retrieved
- Add small delay after `get()` for callback execution

### Dependencies Hanging
- Check for circular dependencies
- Verify all dependency tasks complete
- Use timeouts to prevent infinite waits
- Check task error messages

### Progress Not Updating
- Call `mp.report_progress()` from within the task
- Ensure progress callback is registered before task starts
- Progress values must be between 0.0 and 1.0

---

## Complete Example

```python
import makeparallel as mp
import time

# Task 1: Fetch data with progress
@mp.parallel_with_deps
def fetch_data(source):
    print(f"Fetching from {source}...")
    data = []
    for i in range(5):
        time.sleep(0.2)
        data.append(f"item_{i}")
        mp.report_progress((i + 1) / 5)
    return data

# Task 2: Process data (depends on fetch)
@mp.parallel_with_deps
def process_data(deps):
    data = deps[0]
    print(f"Processing {len(data)} items...")
    result = [item.upper() for item in data]
    return result

# Task 3: Save results (depends on process)
@mp.parallel_with_deps
def save_results(deps):
    processed = deps[0]
    print(f"Saving {len(processed)} items...")
    return f"Saved {len(processed)} items to database"

# Execute pipeline
h1 = fetch_data("API")
h1.on_progress(lambda p: print(f"Fetch progress: {p*100:.0f}%"))
h1.on_complete(lambda r: print(f"Fetched: {len(r)} items"))

h2 = process_data(depends_on=[h1])
h2.on_complete(lambda r: print(f"Processed: {len(r)} items"))

h3 = save_results(depends_on=[h2])
h3.on_complete(lambda r: print(f"Final: {r}"))
h3.on_error(lambda e: print(f"ERROR: {e}"))

# Get final result
final = h3.get()
print(f"\nPipeline complete: {final}")
```

---

## Summary

**Callbacks** provide hooks to react to task events:
- ✅ Monitor progress in real-time
- ✅ Handle completion and errors
- ✅ Integrate with existing systems

**Dependencies** enable complex workflows:
- ✅ Chain tasks together
- ✅ Pass results between tasks
- ✅ Create parallel pipelines

Together, they enable building sophisticated parallel workflows with full observability and control.
