# Quick Wins - Easy High-Impact Improvements

These are the easiest features to implement with the highest user impact.

## 1. Task Timeouts (30 minutes) ⭐⭐⭐

### What
Automatically cancel tasks that run too long.

### Implementation
```rust
// In ParallelWrapper
#[pyo3(signature = (*args, timeout=None, **kwargs))]
fn __call__(
    &self,
    py: Python,
    args: &Bound<'_, PyTuple>,
    timeout: Option<f64>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<AsyncHandle>> {
    // ... existing code ...

    if let Some(timeout_secs) = timeout {
        let cancel_token = cancel_token.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_secs_f64(timeout_secs));
            cancel_token.store(true, Ordering::SeqCst);
        });
    }
}
```

### Usage
```python
@mp.parallel
def long_task():
    time.sleep(100)

# Automatically cancelled after 5 seconds
handle = long_task(timeout=5)
```

### Impact
- Prevents hung tasks
- Better resource management
- Common user request

---

## 2. Result Callbacks (45 minutes) ⭐⭐⭐

### What
Execute callback when task completes (success or failure).

### Implementation
```rust
#[pyclass]
struct AsyncHandle {
    // ... existing fields ...
    on_complete: Arc<Mutex<Option<Py<PyAny>>>>,
    on_error: Arc<Mutex<Option<Py<PyAny>>>>,
}

impl AsyncHandle {
    fn set_callback(&self, py: Python, on_complete: Option<Py<PyAny>>, on_error: Option<Py<PyAny>>) {
        if let Some(cb) = on_complete {
            *self.on_complete.lock().unwrap() = Some(cb);
        }
        if let Some(cb) = on_error {
            *self.on_error.lock().unwrap() = Some(cb);
        }
    }
}

// In thread execution:
let result = func.bind(py).call(...);
match result {
    Ok(val) => {
        if let Some(cb) = on_complete.lock().unwrap().as_ref() {
            let _ = cb.call1(py, (val,));
        }
    }
    Err(e) => {
        if let Some(cb) = on_error.lock().unwrap().as_ref() {
            let _ = cb.call1(py, (e,));
        }
    }
}
```

### Usage
```python
def on_success(result):
    print(f"Success: {result}")

def on_failure(error):
    print(f"Error: {error}")

handle = task()
handle.set_callback(on_complete=on_success, on_error=on_failure)
```

### Impact
- Cleaner async code
- Event-driven architecture
- No blocking for results

---

## 3. Better Error Context (1 hour) ⭐⭐⭐

### What
Preserve full error information including traceback.

### Implementation
```rust
#[pyclass]
struct TaskError {
    #[pyo3(get)]
    task_name: String,
    #[pyo3(get)]
    elapsed_time: f64,
    #[pyo3(get)]
    error_message: String,
    #[pyo3(get)]
    error_type: String,
    traceback: String,
}

// When error occurs:
Err(e) => {
    let task_error = TaskError {
        task_name: func_name.clone(),
        elapsed_time: start.elapsed().as_secs_f64(),
        error_message: e.to_string(),
        error_type: e.get_type(py).name().unwrap().to_string(),
        traceback: e.traceback(py)
            .map(|tb| tb.format().unwrap_or_default())
            .unwrap_or_default(),
    };
    Err(PyErr::from(task_error))
}
```

### Usage
```python
try:
    result = handle.get()
except mp.TaskError as e:
    logger.error(f"Task {e.task_name} failed after {e.elapsed_time}s")
    logger.error(f"Error: {e.error_message}")
    logger.error(f"Traceback:\n{e.traceback}")
```

### Impact
- Much easier debugging
- Better production monitoring
- Essential for reliability

---

## 4. Global Configuration (30 minutes) ⭐⭐

### What
Set defaults globally instead of per-task.

### Implementation
```rust
static GLOBAL_CONFIG: Lazy<Arc<Mutex<GlobalConfig>>> = Lazy::new(|| {
    Arc::new(Mutex::new(GlobalConfig::default()))
});

#[pyclass]
struct GlobalConfig {
    default_timeout: Option<f64>,
    log_level: String,
    profile_all: bool,
    max_concurrent: Option<usize>,
}

#[pyfunction]
fn configure(py: Python, config: &Bound<'_, PyDict>) -> PyResult<()> {
    let mut cfg = GLOBAL_CONFIG.lock().unwrap();

    if let Ok(timeout) = config.get_item("default_timeout") {
        cfg.default_timeout = timeout.extract()?;
    }
    // ... etc
    Ok(())
}
```

### Usage
```python
# Set once globally
mp.configure({
    'default_timeout': 60,
    'log_level': 'DEBUG',
    'profile_all': True,
    'max_concurrent': 100
})

# All tasks use these defaults
@mp.parallel  # Uses default_timeout=60
def task():
    pass
```

### Impact
- Less repetition
- Easier configuration management
- Production-friendly

---

## 5. Task Metadata (20 minutes) ⭐⭐

### What
Attach custom metadata to tasks for tracking.

### Implementation
```rust
#[pyclass]
struct AsyncHandle {
    // ... existing fields ...
    metadata: Arc<Mutex<HashMap<String, Py<PyAny>>>>,
}

#[pymethods]
impl AsyncHandle {
    fn set_metadata(&self, py: Python, key: String, value: Py<PyAny>) {
        self.metadata.lock().unwrap().insert(key, value);
    }

    fn get_metadata(&self, py: Python, key: String) -> Option<Py<PyAny>> {
        self.metadata.lock().unwrap().get(&key).map(|v| v.clone_ref(py))
    }

    fn get_all_metadata(&self, py: Python) -> PyResult<Py<PyDict>> {
        let dict = PyDict::new(py);
        let metadata = self.metadata.lock().unwrap();
        for (k, v) in metadata.iter() {
            dict.set_item(k, v)?;
        }
        Ok(dict.unbind())
    }
}
```

### Usage
```python
handle = process_user_data(user_id=123)
handle.set_metadata('user_id', 123)
handle.set_metadata('job_id', 'abc-123')
handle.set_metadata('priority', 'high')

# Later...
metadata = handle.get_all_metadata()
logger.info(f"Processing job {metadata['job_id']} for user {metadata['user_id']}")
```

### Impact
- Better tracking
- Easier debugging
- Custom monitoring

---

## 6. Progress Reporting (1 hour) ⭐⭐

### What
Report progress from within tasks.

### Implementation
```rust
// Global progress tracking
static TASK_PROGRESS: Lazy<Arc<DashMap<String, f64>>> =
    Lazy::new(|| Arc::new(DashMap::new()));

#[pyfunction]
fn report_progress(task_id: String, progress: f64) -> PyResult<()> {
    TASK_PROGRESS.insert(task_id, progress);
    Ok(())
}

#[pymethods]
impl AsyncHandle {
    fn get_progress(&self) -> f64 {
        TASK_PROGRESS
            .get(&self.task_id)
            .map(|p| *p)
            .unwrap_or(0.0)
    }
}
```

### Usage
```python
@mp.parallel
def long_task(items):
    for i, item in enumerate(items):
        process(item)
        mp.report_progress(i / len(items))
    return "done"

handle = long_task(range(1000))

while not handle.is_ready():
    print(f"Progress: {handle.get_progress() * 100:.1f}%")
    time.sleep(0.5)
```

### Impact
- Better UX
- Monitor long tasks
- User satisfaction

---

## 7. Better Logging (45 minutes) ⭐⭐

### What
Structured logging with context.

### Implementation
```rust
use log::{debug, info, warn, error};

// Enable logging
#[pyfunction]
fn configure_logging(level: String, format: String) -> PyResult<()> {
    env_logger::Builder::new()
        .filter_level(match level.as_str() {
            "DEBUG" => log::LevelFilter::Debug,
            "INFO" => log::LevelFilter::Info,
            "WARN" => log::LevelFilter::Warn,
            "ERROR" => log::LevelFilter::Error,
            _ => log::LevelFilter::Info,
        })
        .init();
    Ok(())
}

// In task execution:
info!("Starting task: {} with args: {:?}", func_name, args);
debug!("Task {} executing in thread {:?}", func_name, thread::current().id());
error!("Task {} failed: {}", func_name, error);
```

### Usage
```python
mp.configure_logging(level='DEBUG', format='json')

# Now all operations are logged
handle = task()  # Logs: "Starting task: task with args: ()"
```

### Impact
- Production debugging
- Audit trail
- Performance analysis

---

## 8. Graceful Shutdown (1 hour) ⭐⭐⭐

### What
Clean shutdown of all running tasks.

### Implementation
```rust
static SHUTDOWN_FLAG: Lazy<Arc<AtomicBool>> =
    Lazy::new(|| Arc::new(AtomicBool::new(false)));

static ACTIVE_HANDLES: Lazy<Arc<Mutex<Vec<Weak<AsyncHandle>>>>> =
    Lazy::new(|| Arc::new(Mutex::new(Vec::new())));

#[pyfunction]
fn shutdown(timeout: Option<f64>, cancel_pending: bool) -> PyResult<()> {
    SHUTDOWN_FLAG.store(true, Ordering::SeqCst);

    let handles = ACTIVE_HANDLES.lock().unwrap();
    let start = Instant::now();
    let timeout_duration = timeout.map(Duration::from_secs_f64);

    for weak_handle in handles.iter() {
        if let Some(handle) = weak_handle.upgrade() {
            if cancel_pending {
                handle.cancel()?;
            } else {
                // Wait for completion
                if let Some(timeout) = timeout_duration {
                    let remaining = timeout.saturating_sub(start.elapsed());
                    handle.wait(Some(remaining.as_secs_f64()))?;
                }
            }
        }
    }

    Ok(())
}
```

### Usage
```python
import atexit

# Register shutdown handler
atexit.register(lambda: mp.shutdown(timeout=30, cancel_pending=True))

# Or manual shutdown
mp.shutdown(timeout=30, cancel_pending=False)  # Wait for tasks
```

### Impact
- Production stability
- Clean shutdown
- No orphaned threads

---

## Implementation Priority

### Week 1 (High Impact, Low Effort)
1. ✅ Task Timeouts - 30 min
2. ✅ Task Metadata - 20 min
3. ✅ Global Configuration - 30 min

**Total: 1.5 hours, 3 features**

### Week 2 (High Impact, Medium Effort)
4. ✅ Result Callbacks - 45 min
5. ✅ Better Error Context - 1 hour
6. ✅ Better Logging - 45 min

**Total: 2.5 hours, 3 features**

### Week 3 (High Impact, More Effort)
7. ✅ Graceful Shutdown - 1 hour
8. ✅ Progress Reporting - 1 hour

**Total: 2 hours, 2 features**

### Total Effort
**6 hours for 8 high-impact features!**

---

## Combined Example

After implementing these, users could write:

```python
import makeParallel as mp

# Global config
mp.configure({
    'default_timeout': 60,
    'log_level': 'INFO',
    'max_concurrent': 100
})

# Register cleanup
import atexit
atexit.register(lambda: mp.shutdown(timeout=30))

# Define task with callbacks
@mp.parallel
def process_batch(batch_id, items):
    for i, item in enumerate(items):
        mp.report_progress(i / len(items))
        process(item)
    return f"Processed {len(items)} items"

def on_success(result):
    logger.info(f"Batch complete: {result}")

def on_error(error):
    logger.error(f"Batch failed: {error.task_name} - {error.error_message}")
    logger.error(f"Traceback:\n{error.traceback}")

# Execute
handle = process_batch(1, items)
handle.set_metadata('user_id', user_id)
handle.set_metadata('job_id', job_id)
handle.set_callback(on_complete=on_success, on_error=on_error)

# Monitor
while not handle.is_ready():
    progress = handle.get_progress()
    metadata = handle.get_all_metadata()
    print(f"Job {metadata['job_id']}: {progress*100:.1f}% complete")
    time.sleep(1)
```

Much cleaner and more powerful! 🚀

---

Would you like me to implement any of these quick wins?
