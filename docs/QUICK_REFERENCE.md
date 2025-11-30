# Quick Reference - Callbacks & Dependencies

## Callbacks

### Setup
```python
import makeparallel as mp

@mp.parallel
def my_task():
    mp.report_progress(0.5)  # Report 50% progress
    return "result"

handle = my_task()
```

### on_complete
```python
handle.on_complete(lambda result: print(f"Done: {result}"))
```

### on_error
```python
handle.on_error(lambda error: print(f"Error: {error}"))
```

### on_progress
```python
handle.on_progress(lambda p: print(f"Progress: {p*100}%"))
```

### Get Result
```python
result = handle.get()  # Blocks until complete, triggers callbacks
```

---

## Dependencies

### Basic Dependency
```python
@mp.parallel_with_deps
def task1():
    return "data"

@mp.parallel_with_deps
def task2(deps):
    # deps[0] contains result from task1
    return f"processed {deps[0]}"

h1 = task1()
h2 = task2(depends_on=[h1])  # Waits for task1
result = h2.get()
```

### Multiple Dependencies
```python
@mp.parallel_with_deps
def combine(deps):
    # deps is tuple of all dependency results
    return deps[0] + deps[1] + deps[2]

h1 = task_a()
h2 = task_b()
h3 = task_c()

h_final = combine(depends_on=[h1, h2, h3])
```

### Chain
```python
h1 = step1()
h2 = step2(depends_on=[h1])
h3 = step3(depends_on=[h2])
h4 = step4(depends_on=[h3])

final = h4.get()  # Executes full chain
```

---

## Common Patterns

### Progress Bar
```python
@mp.parallel
def download(url):
    for i in range(100):
        download_chunk(url, i)
        mp.report_progress(i / 100)
    return "done"

handle = download("http://example.com/file")
handle.on_progress(lambda p: progress_bar.update(p))
```

### Error Logging
```python
@mp.parallel
def risky_task():
    # might fail
    return process_data()

handle = risky_task()
handle.on_error(lambda e: logger.error(f"Task failed: {e}"))
handle.on_complete(lambda r: logger.info(f"Success: {r}"))
```

### Pipeline
```python
@mp.parallel_with_deps
def fetch():
    return get_data()

@mp.parallel_with_deps
def process(deps):
    return transform(deps[0])

@mp.parallel_with_deps
def save(deps):
    return write_db(deps[0])

h1 = fetch()
h2 = process(depends_on=[h1])
h3 = save(depends_on=[h2])

final = h3.get()  # Executes pipeline
```

### Parallel + Merge
```python
# Parallel execution
h1 = fetch_users()
h2 = fetch_products()
h3 = fetch_orders()

# Merge results
@mp.parallel_with_deps
def merge(deps):
    users, products, orders = deps
    return generate_report(users, products, orders)

h_report = merge(depends_on=[h1, h2, h3])
```

---

## Tips

### Progress Reporting
```python
# Report at regular intervals
total = len(items)
for i, item in enumerate(items):
    process(item)
    if i % 10 == 0:  # Every 10 items
        mp.report_progress(i / total)

mp.report_progress(1.0)  # Always report 100% at end
```

### Error Handling in Callbacks
```python
def safe_callback(result):
    try:
        process(result)
    except Exception as e:
        log_error(e)

handle.on_complete(safe_callback)
```

### Timeout for Dependencies
```python
h2 = task2(depends_on=[h1], timeout=60.0)  # 60 second timeout
```

---

## Complete Example

```python
import makeparallel as mp
import time

# Define tasks
@mp.parallel_with_deps
def fetch_data():
    print("Fetching...")
    for i in range(5):
        time.sleep(0.1)
        mp.report_progress(i / 5)
    return ["item1", "item2", "item3"]

@mp.parallel_with_deps
def process_data(deps):
    print("Processing...")
    data = deps[0]
    return [x.upper() for x in data]

@mp.parallel_with_deps
def save_data(deps):
    print("Saving...")
    processed = deps[0]
    return f"Saved {len(processed)} items"

# Execute pipeline
h1 = fetch_data()
h1.on_progress(lambda p: print(f"Fetch: {p*100:.0f}%"))

h2 = process_data(depends_on=[h1])
h2.on_complete(lambda r: print(f"Processed: {r}"))

h3 = save_data(depends_on=[h2])
h3.on_complete(lambda r: print(f"Final: {r}"))
h3.on_error(lambda e: print(f"ERROR: {e}"))

# Get result
result = h3.get()
print(f"Pipeline result: {result}")
```

---

## Troubleshooting

### Callbacks not firing?
- Ensure you call `handle.get()` or `handle.wait()`
- Add `time.sleep(0.1)` after `get()` for callbacks to execute

### Dependencies hanging?
- Check for circular dependencies
- Verify all dependencies complete
- Use `timeout` parameter
- Check error messages

### Progress not updating?
- Call `mp.report_progress()` from within the task
- Register callback before calling `get()`
- Values must be 0.0 to 1.0

---

**See full documentation in `CALLBACKS_AND_DEPENDENCIES.md`**
