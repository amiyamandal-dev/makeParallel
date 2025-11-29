# Threading Best Practices & Architecture Philosophy

## Our Philosophy: A Focused, Scalable Threading Library

makeParallel is designed to be **an excellent threading library** that scales easily, NOT a complex ETL/workflow orchestration tool.

### What We Are
✅ High-performance parallel execution
✅ True GIL-free parallelism with Rust
✅ Simple, intuitive API
✅ Production-ready error handling
✅ Graceful resource management
✅ Minimal dependencies

### What We Are NOT
❌ A workflow orchestration engine (use Airflow, Prefect)
❌ A distributed task queue (use Celery, RQ)
❌ A DAG execution engine (use Dask, Luigi)
❌ A data pipeline framework (use Spark, Flink)
❌ A scheduling system (use APScheduler, Cron)

## Core Design Principles

### 1. Simplicity Over Features
**Keep the API simple and focused on threading.**

```python
# ✅ GOOD - Simple, clear threading
@mp.parallel
def process_item(item):
    return expensive_operation(item)

handles = [process_item(item) for item in items]
results = [h.get() for h in handles]
```

```python
# ❌ BAD - Don't add complex workflow features
@mp.parallel_dag  # NO!
@mp.with_retry_backoff_circuit_breaker  # NO!
@mp.schedule(cron="0 0 * * *")  # NO!
def complex_task():
    pass
```

### 2. Scalability Through Simplicity
**Scale by doing one thing really well.**

Focus on:
- Thread pool management
- Task execution
- Error handling
- Resource cleanup
- Performance monitoring

Let other tools handle:
- Task scheduling
- Workflow dependencies
- Distributed execution
- Data transformation

### 3. Composability Over Complexity
**Build small, composable pieces.**

```python
# ✅ GOOD - Compose with other tools
import makeParallel as mp
from apscheduler.schedulers.background import BackgroundScheduler

scheduler = BackgroundScheduler()

@scheduler.scheduled_job('cron', hour=2)
def nightly_job():
    # Use mp for parallel execution
    handles = [mp.parallel(process_batch)(batch) for batch in batches]
    results = [h.get() for h in handles]
```

## Threading Best Practices

### Task Design

#### ✅ DO: CPU-Bound Tasks
```python
@mp.parallel
def compute_hash(data):
    """Perfect use case - CPU intensive"""
    return hashlib.sha256(data).hexdigest()

@mp.parallel
def process_image(image_path):
    """Perfect use case - CPU intensive"""
    img = load_image(image_path)
    return apply_filters(img)
```

#### ❌ DON'T: Pure I/O Tasks
```python
# Use asyncio instead!
@mp.parallel  # ❌ Wrong tool
async def fetch_url(url):
    async with aiohttp.ClientSession() as session:
        return await session.get(url)
```

### Error Handling

#### ✅ DO: Capture Context
```python
@mp.parallel
def risky_operation(item_id):
    try:
        return process(item_id)
    except Exception as e:
        # Errors already have context!
        raise  # TaskError will include task_id, timing, etc.

handle = risky_operation(123)
handle.set_metadata('user_id', user_id)

try:
    result = handle.get()
except Exception as e:
    logger.error(f"Failed for user {user_id}: {e}")
```

#### ❌ DON'T: Swallow Errors
```python
@mp.parallel
def bad_error_handling(item):
    try:
        return process(item)
    except:  # ❌ Don't catch all exceptions
        return None  # ❌ Loses error information
```

### Resource Management

#### ✅ DO: Configure Thread Pool
```python
import makeParallel as mp

# Configure once at startup
mp.configure_thread_pool(
    num_threads=8,  # Match your workload
    stack_size=2*1024*1024  # 2MB per thread
)

# Use throughout your app
@mp.parallel
def work(item):
    return process(item)
```

#### ✅ DO: Clean Shutdown
```python
import atexit
import makeParallel as mp

# Register cleanup
atexit.register(lambda: mp.shutdown(timeout_secs=30, cancel_pending=True))

# Or in your main:
def main():
    try:
        run_application()
    finally:
        mp.shutdown(timeout_secs=30, cancel_pending=True)
```

#### ❌ DON'T: Create Too Many Threads
```python
# ❌ BAD - Creates 1000 threads!
handles = [long_task() for _ in range(1000)]

# ✅ GOOD - Use batching
from itertools import islice

def batch(iterable, n):
    it = iter(iterable)
    while chunk := list(islice(it, n)):
        yield chunk

# Process in batches
for batch_items in batch(range(1000), 50):
    handles = [process(item) for item in batch_items]
    results = [h.get() for h in handles]
```

### Monitoring and Observability

#### ✅ DO: Use Metadata for Tracking
```python
@mp.parallel
def process_user_data(user_id, data):
    return expensive_computation(data)

handle = process_user_data(user_id, data)
handle.set_metadata('user_id', str(user_id))
handle.set_metadata('request_id', request_id)
handle.set_metadata('priority', 'high')

# Later, for monitoring
metadata = handle.get_all_metadata()
logger.info(f"Processing user {metadata['user_id']}, request {metadata['request_id']}")
```

#### ✅ DO: Monitor Performance
```python
import makeParallel as mp

# Automatic profiling with @parallel
@mp.parallel
def tracked_task(n):
    return compute(n)

# Run tasks
handles = [tracked_task(i) for i in range(100)]
results = [h.get() for h in handles]

# Check metrics
metrics = mp.get_all_metrics()
if 'tracked_task' in metrics:
    m = metrics['tracked_task']
    avg_time = m['average_execution_time_ms']
    success_rate = m['completed_tasks'] / m['total_tasks'] * 100

    if avg_time > 1000:
        logger.warning(f"Task taking too long: {avg_time}ms")
    if success_rate < 95:
        logger.error(f"High failure rate: {100-success_rate}%")
```

## Scalability Patterns

### Pattern 1: Producer-Consumer

```python
import makeParallel as mp
from queue import Queue

def producer_consumer_pattern(items):
    """Process items in parallel with bounded concurrency"""
    max_concurrent = 20
    semaphore = threading.Semaphore(max_concurrent)

    def work(item):
        with semaphore:
            handle = process_item(item)
            return handle.get()

    with concurrent.futures.ThreadPoolExecutor() as executor:
        results = list(executor.map(work, items))

    return results
```

### Pattern 2: Map-Reduce

```python
import makeParallel as mp

def map_reduce_pattern(data_chunks):
    """Parallel map, sequential reduce"""

    # Map phase - parallel
    @mp.parallel
    def map_func(chunk):
        return [process(item) for item in chunk]

    map_handles = [map_func(chunk) for chunk in data_chunks]
    mapped_results = [h.get() for h in map_handles]

    # Reduce phase - sequential
    final_result = reduce(combine, mapped_results)
    return final_result
```

### Pattern 3: Pipeline

```python
import makeParallel as mp

def pipeline_pattern(items):
    """Multi-stage pipeline processing"""

    # Stage 1: Parallel preprocessing
    @mp.parallel
    def preprocess(item):
        return clean(item)

    preprocessed_handles = [preprocess(item) for item in items]
    preprocessed = [h.get() for h in preprocessed_handles]

    # Stage 2: Parallel main processing
    @mp.parallel
    def main_process(item):
        return transform(item)

    processed_handles = [main_process(item) for item in preprocessed]
    processed = [h.get() for h in processed_handles]

    # Stage 3: Sequential aggregation
    return aggregate(processed)
```

## Integration with Other Tools

### With Flask
```python
from flask import Flask
import makeParallel as mp

app = Flask(__name__)

# Configure on startup
mp.configure_thread_pool(num_threads=8)

@app.route('/process', methods=['POST'])
def process_endpoint():
    data = request.json

    @mp.parallel
    def process_request(data):
        return expensive_operation(data)

    handle = process_request(data)
    handle.set_metadata('request_id', request.id)

    # Option 1: Wait for result
    result = handle.get()
    return jsonify(result)

    # Option 2: Return task ID for polling
    return jsonify({'task_id': handle.get_task_id()})

@app.teardown_appcontext
def shutdown_threads(exception=None):
    mp.shutdown(timeout_secs=10)
```

### With FastAPI
```python
from fastapi import FastAPI, BackgroundTasks
import makeParallel as mp

app = FastAPI()

@app.on_event("startup")
async def startup():
    mp.configure_thread_pool(num_threads=16)

@app.on_event("shutdown")
async def shutdown():
    mp.shutdown(timeout_secs=30)

@app.post("/compute")
async def compute(data: dict, background_tasks: BackgroundTasks):
    @mp.parallel
    def cpu_bound_work(data):
        return complex_computation(data)

    handle = cpu_bound_work(data)

    # For CPU-bound work in web apps
    result = handle.get()
    return {"result": result}
```

### With Celery (for distribution)
```python
from celery import Celery
import makeParallel as mp

app = Celery('tasks', broker='redis://localhost')

@app.task
def distributed_batch_processing(batch_items):
    """Celery for distribution, mp for local parallelism"""

    @mp.parallel
    def process_item(item):
        return expensive_computation(item)

    # Process this batch in parallel locally
    handles = [process_item(item) for item in batch_items]
    results = [h.get() for h in handles]

    return results
```

## What NOT to Add

### ❌ NO: Workflow Orchestration
Don't add:
- Task dependencies / DAGs
- Conditional execution
- Retry policies beyond simple retry
- State machines

**Why**: Tools like Airflow, Prefect, Temporal excel at this.

**Instead**: Focus on fast, reliable task execution.

### ❌ NO: Distributed Execution
Don't add:
- Multi-machine coordination
- Distributed task queues
- Network communication
- Service discovery

**Why**: Tools like Celery, Dask, Ray are built for this.

**Instead**: Focus on single-machine parallelism.

### ❌ NO: Data Processing
Don't add:
- DataFrame operations
- SQL queries
- Stream processing
- Data transformations

**Why**: Tools like Pandas, Polars, Spark handle data.

**Instead**: Focus on executing user functions in parallel.

### ❌ NO: Complex Scheduling
Don't add:
- Cron expressions
- Recurring tasks
- Job queues
- Priority scheduling beyond basic priorities

**Why**: APScheduler, Celery Beat, etc. do this better.

**Instead**: Focus on immediate task execution.

## What TO Add (If Anything)

### ✅ YES: Core Threading Features
- Better thread pool management
- More metrics and monitoring
- Resource limits (memory, CPU)
- Better cancellation
- Async/await integration

### ✅ YES: Developer Experience
- Better error messages
- More examples
- Performance optimization
- Testing utilities
- Documentation

### ✅ YES: Production Readiness
- Logging integration
- Health checks
- Metrics export (Prometheus, StatsD)
- Graceful degradation
- Circuit breakers

## Measuring Success

### Good Metrics
- **Throughput**: Tasks completed per second
- **Latency**: Average task execution time
- **Reliability**: Success rate, error rate
- **Resource Usage**: CPU, memory utilization
- **Scalability**: Performance with increasing load

### Bad Metrics
- Number of features
- Lines of code
- Complexity of API
- Number of integrations

## Summary

**makeParallel should be:**
- The best way to run Python functions in parallel
- Simple, fast, reliable
- Easy to integrate
- Production-ready
- Focused on threading

**makeParallel should NOT be:**
- A workflow engine
- A distributed system
- A data processing framework
- A scheduling system
- All-in-one solution

**Our Motto:**
> "Do one thing extremely well: parallel execution of Python functions"

When in doubt, ask:
1. Is this about running functions in parallel?
2. Does it make threading easier or more reliable?
3. Can existing tools do this better?

If #3 is yes, don't add it. Stay focused. 🎯
