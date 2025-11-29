use pyo3::IntoPyObjectExt;
use pyo3::prelude::*;
use pyo3::types::{PyCFunction, PyDict, PyTuple};
use pyo3::wrap_pyfunction;
use std::collections::{BinaryHeap, HashMap};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use std::cmp::Ordering as CmpOrdering;

// Optimized imports
use crossbeam::channel::{Receiver as CrossbeamReceiver, Sender as CrossbeamSender, unbounded};
use dashmap::DashMap;
use rayon::prelude::*;
use once_cell::sync::Lazy;

// =============================================================================
// ERROR HANDLING
// =============================================================================

/// Enhanced error information for task failures
#[pyclass]
#[derive(Clone)]
struct TaskError {
    #[pyo3(get)]
    task_name: String,
    #[pyo3(get)]
    elapsed_time: f64,
    #[pyo3(get)]
    error_message: String,
    #[pyo3(get)]
    error_type: String,
    #[pyo3(get)]
    task_id: String,
}

#[pymethods]
impl TaskError {
    fn __str__(&self) -> String {
        format!(
            "TaskError in '{}' (task_id: {}, elapsed: {}s): {} ({})",
            self.task_name, self.task_id, self.elapsed_time, self.error_message, self.error_type
        )
    }

    fn __repr__(&self) -> String {
        self.__str__()
    }
}

// =============================================================================
// GRACEFUL SHUTDOWN
// =============================================================================

/// Global shutdown flag
static SHUTDOWN_FLAG: Lazy<Arc<AtomicBool>> = Lazy::new(|| Arc::new(AtomicBool::new(false)));

/// Active task handles for shutdown
static ACTIVE_TASKS: Lazy<Arc<Mutex<Vec<String>>>> = Lazy::new(|| Arc::new(Mutex::new(Vec::new())));

/// Task ID counter
static TASK_ID_COUNTER: Lazy<Arc<AtomicU64>> = Lazy::new(|| Arc::new(AtomicU64::new(0)));

/// Check if shutdown is requested
fn is_shutdown_requested() -> bool {
    SHUTDOWN_FLAG.load(Ordering::SeqCst)
}

/// Register a task as active
fn register_task(task_id: String) {
    ACTIVE_TASKS.lock().unwrap().push(task_id);
}

/// Unregister a task
fn unregister_task(task_id: &str) {
    let mut tasks = ACTIVE_TASKS.lock().unwrap();
    tasks.retain(|id| id != task_id);
}

/// Get active task count
#[pyfunction]
fn get_active_task_count() -> usize {
    ACTIVE_TASKS.lock().unwrap().len()
}

/// Initiate graceful shutdown
#[pyfunction]
fn shutdown(timeout_secs: Option<f64>, cancel_pending: bool) -> PyResult<bool> {
    println!("Initiating graceful shutdown...");
    SHUTDOWN_FLAG.store(true, Ordering::SeqCst);

    let start = Instant::now();
    let timeout = timeout_secs.map(Duration::from_secs_f64).unwrap_or(Duration::from_secs(30));

    // Stop priority worker
    let _ = stop_priority_worker();

    // Wait for active tasks
    loop {
        let active_count = get_active_task_count();
        if active_count == 0 {
            println!("All tasks completed. Shutdown successful.");
            return Ok(true);
        }

        if start.elapsed() >= timeout {
            println!("Shutdown timeout reached. {} tasks still active.", active_count);
            if cancel_pending {
                println!("Cancelling remaining tasks...");
                // Tasks will check shutdown flag and exit
            }
            return Ok(false);
        }

        thread::sleep(Duration::from_millis(100));
    }
}

/// Reset shutdown flag (for testing)
#[pyfunction]
fn reset_shutdown() -> PyResult<()> {
    SHUTDOWN_FLAG.store(false, Ordering::SeqCst);
    Ok(())
}

// =============================================================================
// BACKPRESSURE AND RATE LIMITING
// =============================================================================

/// Global concurrent task limit
static MAX_CONCURRENT_TASKS: Lazy<Arc<Mutex<Option<usize>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

/// Set maximum concurrent tasks
#[pyfunction]
fn set_max_concurrent_tasks(max_tasks: usize) -> PyResult<()> {
    *MAX_CONCURRENT_TASKS.lock().unwrap() = Some(max_tasks);
    Ok(())
}

/// Wait for available slot (backpressure)
fn wait_for_slot() {
    if let Some(max) = *MAX_CONCURRENT_TASKS.lock().unwrap() {
        while get_active_task_count() >= max {
            thread::sleep(Duration::from_millis(10));
        }
    }
}

// =============================================================================
// MEMORY MONITORING
// =============================================================================

/// Global memory limit (percentage)
static MEMORY_LIMIT_PERCENT: Lazy<Arc<Mutex<Option<f64>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

/// Configure memory limit
#[pyfunction]
fn configure_memory_limit(max_memory_percent: f64) -> PyResult<()> {
    if max_memory_percent <= 0.0 || max_memory_percent > 100.0 {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            "max_memory_percent must be between 0 and 100"
        ));
    }
    *MEMORY_LIMIT_PERCENT.lock().unwrap() = Some(max_memory_percent);
    Ok(())
}

/// Check if memory usage is acceptable
fn check_memory_ok() -> bool {
    if let Some(_limit) = *MEMORY_LIMIT_PERCENT.lock().unwrap() {
        // In a real implementation, would check actual memory usage
        // For now, always return true
        // TODO: Add actual memory checking with sysinfo crate
        true
    } else {
        true
    }
}

// =============================================================================
// PROGRESS TRACKING
// =============================================================================

/// Global progress tracking
static TASK_PROGRESS_MAP: Lazy<Arc<DashMap<String, f64>>> =
    Lazy::new(|| Arc::new(DashMap::new()));

/// Report progress from within a task
#[pyfunction]
fn report_progress(task_id: String, progress: f64) -> PyResult<()> {
    if progress < 0.0 || progress > 1.0 {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            "progress must be between 0.0 and 1.0"
        ));
    }
    TASK_PROGRESS_MAP.insert(task_id, progress);
    Ok(())
}

// =============================================================================
// THREAD POOL CONFIGURATION
// =============================================================================

/// Global thread pool configuration
static CUSTOM_THREAD_POOL: Lazy<Arc<Mutex<Option<rayon::ThreadPool>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

/// Configure the global thread pool size
#[pyfunction]
#[pyo3(signature = (num_threads=None, stack_size=None))]
fn configure_thread_pool(py: Python, num_threads: Option<usize>, stack_size: Option<usize>) -> PyResult<()> {
    py.detach(|| {
        let mut builder = rayon::ThreadPoolBuilder::new();

        if let Some(threads) = num_threads {
            builder = builder.num_threads(threads);
        }

        if let Some(stack) = stack_size {
            builder = builder.stack_size(stack);
        }

        let pool = builder.build().map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("Failed to build thread pool: {}", e))
        })?;

        *CUSTOM_THREAD_POOL.lock().unwrap() = Some(pool);
        Ok(())
    })
}

/// Get current thread pool info
#[pyfunction]
fn get_thread_pool_info(py: Python) -> PyResult<Py<PyDict>> {
    let dict = PyDict::new(py);
    let pool = CUSTOM_THREAD_POOL.lock().unwrap();

    if let Some(p) = pool.as_ref() {
        dict.set_item("configured", true)?;
        dict.set_item("current_num_threads", p.current_num_threads())?;
    } else {
        dict.set_item("configured", false)?;
        dict.set_item("current_num_threads", rayon::current_num_threads())?;
    }

    Ok(dict.unbind())
}

// =============================================================================
// PRIORITY QUEUE IMPLEMENTATION
// =============================================================================

/// Priority task wrapper
struct PriorityTask {
    priority: i32,
    func: Py<PyAny>,
    args: Py<PyTuple>,
    kwargs: Option<Py<PyDict>>,
    sender: CrossbeamSender<PyResult<Py<PyAny>>>,
}

impl Eq for PriorityTask {}

impl PartialEq for PriorityTask {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}

impl PartialOrd for PriorityTask {
    fn partial_cmp(&self, other: &Self) -> Option<CmpOrdering> {
        Some(self.cmp(other))
    }
}

impl Ord for PriorityTask {
    fn cmp(&self, other: &Self) -> CmpOrdering {
        // Higher priority values come first
        self.priority.cmp(&other.priority)
    }
}

/// Global priority queue
static PRIORITY_QUEUE: Lazy<Arc<Mutex<BinaryHeap<PriorityTask>>>> =
    Lazy::new(|| Arc::new(Mutex::new(BinaryHeap::new())));

/// Worker thread flag
static PRIORITY_WORKER_RUNNING: Lazy<Arc<AtomicBool>> =
    Lazy::new(|| Arc::new(AtomicBool::new(false)));

/// Start the priority queue worker
#[pyfunction]
fn start_priority_worker(py: Python) -> PyResult<()> {
    if PRIORITY_WORKER_RUNNING.load(Ordering::SeqCst) {
        return Ok(());
    }

    PRIORITY_WORKER_RUNNING.store(true, Ordering::SeqCst);

    py.detach(|| {
        thread::spawn(move || {
            while PRIORITY_WORKER_RUNNING.load(Ordering::SeqCst) {
                let task_opt = {
                    let mut queue = PRIORITY_QUEUE.lock().unwrap();
                    queue.pop()
                };

                if let Some(task) = task_opt {
                    Python::attach(|py| {
                        let result = task.func
                            .bind(py)
                            .call(task.args.bind(py), task.kwargs.as_ref().map(|k| k.bind(py)));

                        let to_send = match result {
                            Ok(val) => Ok(val.unbind()),
                            Err(e) => Err(e),
                        };

                        let _ = task.sender.send(to_send);
                    });
                } else {
                    thread::sleep(Duration::from_millis(10));
                }
            }
        })
    });

    Ok(())
}

/// Stop the priority queue worker
#[pyfunction]
fn stop_priority_worker() -> PyResult<()> {
    PRIORITY_WORKER_RUNNING.store(false, Ordering::SeqCst);
    Ok(())
}

// =============================================================================
// PERFORMANCE PROFILING
// =============================================================================

/// Performance metrics
#[pyclass]
#[derive(Clone)]
struct PerformanceMetrics {
    #[pyo3(get)]
    total_tasks: u64,
    #[pyo3(get)]
    completed_tasks: u64,
    #[pyo3(get)]
    failed_tasks: u64,
    #[pyo3(get)]
    total_execution_time_ms: f64,
    #[pyo3(get)]
    average_execution_time_ms: f64,
}

/// Global metrics tracker
static METRICS: Lazy<Arc<Mutex<HashMap<String, PerformanceMetrics>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

static TASK_COUNTER: Lazy<Arc<AtomicU64>> = Lazy::new(|| Arc::new(AtomicU64::new(0)));
static COMPLETED_COUNTER: Lazy<Arc<AtomicU64>> = Lazy::new(|| Arc::new(AtomicU64::new(0)));
static FAILED_COUNTER: Lazy<Arc<AtomicU64>> = Lazy::new(|| Arc::new(AtomicU64::new(0)));

/// Record task execution
fn record_task_execution(name: &str, duration_ms: f64, success: bool) {
    TASK_COUNTER.fetch_add(1, Ordering::SeqCst);

    if success {
        COMPLETED_COUNTER.fetch_add(1, Ordering::SeqCst);
    } else {
        FAILED_COUNTER.fetch_add(1, Ordering::SeqCst);
    }

    let mut metrics = METRICS.lock().unwrap();
    let entry = metrics.entry(name.to_string()).or_insert(PerformanceMetrics {
        total_tasks: 0,
        completed_tasks: 0,
        failed_tasks: 0,
        total_execution_time_ms: 0.0,
        average_execution_time_ms: 0.0,
    });

    entry.total_tasks += 1;
    if success {
        entry.completed_tasks += 1;
    } else {
        entry.failed_tasks += 1;
    }
    entry.total_execution_time_ms += duration_ms;
    entry.average_execution_time_ms = entry.total_execution_time_ms / entry.total_tasks as f64;
}

/// Get performance metrics for a specific function
#[pyfunction]
fn get_metrics(name: String) -> PyResult<Option<PerformanceMetrics>> {
    let metrics = METRICS.lock().unwrap();
    Ok(metrics.get(&name).cloned())
}

/// Get all performance metrics
#[pyfunction]
fn get_all_metrics(py: Python) -> PyResult<Py<PyDict>> {
    let dict = PyDict::new(py);
    let metrics = METRICS.lock().unwrap();

    for (name, metric) in metrics.iter() {
        let metric_dict = PyDict::new(py);
        metric_dict.set_item("total_tasks", metric.total_tasks)?;
        metric_dict.set_item("completed_tasks", metric.completed_tasks)?;
        metric_dict.set_item("failed_tasks", metric.failed_tasks)?;
        metric_dict.set_item("total_execution_time_ms", metric.total_execution_time_ms)?;
        metric_dict.set_item("average_execution_time_ms", metric.average_execution_time_ms)?;
        dict.set_item(name.as_str(), metric_dict)?;
    }

    dict.set_item("_global_total", TASK_COUNTER.load(Ordering::SeqCst))?;
    dict.set_item("_global_completed", COMPLETED_COUNTER.load(Ordering::SeqCst))?;
    dict.set_item("_global_failed", FAILED_COUNTER.load(Ordering::SeqCst))?;

    Ok(dict.unbind())
}

/// Reset all metrics
#[pyfunction]
fn reset_metrics() -> PyResult<()> {
    METRICS.lock().unwrap().clear();
    TASK_COUNTER.store(0, Ordering::SeqCst);
    COMPLETED_COUNTER.store(0, Ordering::SeqCst);
    FAILED_COUNTER.store(0, Ordering::SeqCst);
    Ok(())
}

// Helper wrapper that supports the descriptor protocol for methods
#[pyclass]
struct MethodWrapper {
    #[allow(dead_code)]
    func: Py<PyAny>,
    wrapper: Py<PyAny>,
}

#[pymethods]
impl MethodWrapper {
    #[pyo3(signature = (*args, **kwargs))]
    fn __call__(
        &self,
        py: Python,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        self.wrapper.bind(py).call(args, kwargs).map(|r| r.unbind())
    }

    fn __get__(
        &self,
        py: Python,
        obj: &Bound<'_, PyAny>,
        _objtype: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        if obj.is_none() {
            // Unbound method access, return self
            return Ok(self.wrapper.clone_ref(py));
        }

        // Bound method access, create a partial with obj as first argument
        let functools = py.import("functools")?;
        let partial = functools.getattr("partial")?;
        partial
            .call1((self.wrapper.bind(py), obj))
            .map(|r| r.unbind())
    }
}

// 1. Timer Decorator
#[pyfunction]
fn timer(py: Python, func: Py<PyAny>) -> PyResult<Py<PyAny>> {
    let func_clone = func.clone_ref(py);
    let wrapper = move |args: &Bound<'_, PyTuple>,
                        kwargs: Option<&Bound<'_, PyDict>>|
          -> PyResult<Py<PyAny>> {
        let py = args.py();
        let start = Instant::now();
        let result = func_clone.bind(py).call(args, kwargs)?;
        let duration = start.elapsed();
        println!("Execution took: {:?}", duration);
        Ok(result.unbind())
    };
    let wrapped = PyCFunction::new_closure(py, None, None, wrapper)?;

    // Wrap in MethodWrapper to support methods
    let method_wrapper = Py::new(
        py,
        MethodWrapper {
            func: func.clone_ref(py),
            wrapper: wrapped.into(),
        },
    )?;
    Ok(method_wrapper.into())
}

// 2. Log Calls Decorator
#[pyfunction]
fn log_calls(py: Python, func: Py<PyAny>) -> PyResult<Py<PyAny>> {
    let func_clone = func.clone_ref(py);
    let wrapper = move |args: &Bound<'_, PyTuple>,
                        kwargs: Option<&Bound<'_, PyDict>>|
          -> PyResult<Py<PyAny>> {
        let py = args.py();
        let args_repr: Vec<String> = args
            .iter()
            .map(|arg| arg.repr().unwrap().to_str().unwrap().to_string())
            .collect();
        let kwargs_repr: Option<String> = kwargs.map(|d| {
            d.iter()
                .map(|(k, v)| format!("{}={}", k, v.repr().unwrap().to_str().unwrap()))
                .collect::<Vec<_>>()
                .join(", ")
        });

        print!(
            "Calling '{}' with args: ({})",
            func_clone
                .bind(py)
                .getattr("__name__")?
                .extract::<String>()?,
            args_repr.join(", ")
        );
        if let Some(s) = kwargs_repr {
            if !s.is_empty() {
                print!(", kwargs: {{{}}}", s);
            }
        }
        println!();

        let result = func_clone.bind(py).call(args, kwargs)?;
        println!(
            "Function '{}' returned: {}",
            func_clone
                .bind(py)
                .getattr("__name__")?
                .extract::<String>()?,
            result.repr()?.to_str()?
        );
        Ok(result.unbind())
    };

    let wrapped = PyCFunction::new_closure(py, None, None, wrapper)?;

    // Wrap in MethodWrapper to support methods
    let method_wrapper = Py::new(
        py,
        MethodWrapper {
            func: func.clone_ref(py),
            wrapper: wrapped.into(),
        },
    )?;
    Ok(method_wrapper.into())
}

// 3. Call Counter Decorator (as a PyClass)
#[pyclass(name = "CallCounter")]
struct CallCounter {
    func: Py<PyAny>,
    call_count: Arc<Mutex<i32>>,
}

#[pymethods]
impl CallCounter {
    #[new]
    fn new(func: Py<PyAny>) -> Self {
        CallCounter {
            func,
            call_count: Arc::new(Mutex::new(0)),
        }
    }

    #[pyo3(signature = (*args, **kwargs))]
    fn __call__(
        &self,
        py: Python,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        let mut count = self.call_count.lock().unwrap();
        *count += 1;
        Ok(self.func.bind(py).call(args, kwargs)?.unbind())
    }

    #[getter]
    fn get_call_count(&self) -> PyResult<i32> {
        Ok(*self.call_count.lock().unwrap())
    }

    fn reset(&self) -> PyResult<()> {
        *self.call_count.lock().unwrap() = 0;
        Ok(())
    }

    fn __get__(
        slf: PyRef<'_, Self>,
        obj: &Bound<'_, PyAny>,
        _objtype: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        if obj.is_none() {
            // Unbound method access, return self
            let py = slf.py();
            return Ok(slf.into_bound_py_any(py)?.unbind());
        }

        // Bound method access, create a BoundMethod wrapper
        let py = slf.py();
        let call_count_clone = slf.call_count.clone();
        let decorator = slf.into_bound_py_any(py)?.unbind();
        let bound_method = Py::new(
            py,
            BoundMethod {
                obj: obj.clone().unbind(),
                decorator,
                call_count: call_count_clone,
            },
        )?;
        Ok(bound_method.into())
    }
}

// Helper class for bound methods from CallCounter
#[pyclass]
struct BoundMethod {
    obj: Py<PyAny>,
    decorator: Py<PyAny>,
    call_count: Arc<Mutex<i32>>,
}

#[pymethods]
impl BoundMethod {
    #[pyo3(signature = (*args, **kwargs))]
    fn __call__(
        &self,
        py: Python,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        // Create new tuple with obj as first arg
        let mut new_args = vec![self.obj.bind(py).clone()];
        for arg in args.iter() {
            new_args.push(arg.clone());
        }
        let new_tuple = PyTuple::new(py, new_args)?;
        self.decorator
            .bind(py)
            .call(new_tuple, kwargs)
            .map(|r| r.unbind())
    }

    #[getter]
    fn get_call_count(&self) -> PyResult<i32> {
        Ok(*self.call_count.lock().unwrap())
    }
}

// 4. Retry Decorator
#[pyfunction]
#[pyo3(signature = (*, max_retries=3))]
fn retry(_py: Python<'_>, max_retries: usize) -> PyResult<Py<PyAny>> {
    let factory = move |py: Python<'_>, func: Py<PyAny>| -> PyResult<Py<PyAny>> {
        let wrapper = move |args: &Bound<'_, PyTuple>,
                            kwargs: Option<&Bound<'_, PyDict>>|
              -> PyResult<Py<PyAny>> {
            let py = args.py();
            let mut last_err = None;
            for attempt in 0..=max_retries {
                match func.bind(py).call(args, kwargs) {
                    Ok(res) => return Ok(res.unbind()),
                    Err(e) => {
                        println!("Attempt {} failed: {:?}", attempt + 1, e.to_string());
                        last_err = Some(e);
                        thread::sleep(Duration::from_millis(50)); // Small delay
                    }
                }
            }
            Err(last_err.unwrap())
        };
        let wrapped = PyCFunction::new_closure(py, None, None, wrapper)?;
        Ok(wrapped.into())
    };

    // This creates a decorator that accepts arguments
    let decorator = PyCFunction::new_closure(
        _py,
        None,
        None,
        move |args: &Bound<'_, PyTuple>, _kwargs: Option<&Bound<'_, PyDict>>| {
            // The real function to be decorated is the first argument
            let func = args.get_item(0)?.unbind();
            factory(args.py(), func)
        },
    )?;
    Ok(decorator.into())
}

// 5. Memoize Decorator
#[pyfunction]
fn memoize(py: Python, func: Py<PyAny>) -> PyResult<Py<PyAny>> {
    let cache: Arc<Mutex<HashMap<String, Py<PyAny>>>> = Arc::new(Mutex::new(HashMap::new()));

    let wrapper = move |args: &Bound<'_, PyTuple>,
                        kwargs: Option<&Bound<'_, PyDict>>|
          -> PyResult<Py<PyAny>> {
        let py = args.py();

        // Create a cache key from arguments
        let mut key_parts: Vec<String> = vec![];
        for arg in args.iter() {
            key_parts.push(arg.repr()?.to_str()?.to_string());
        }
        if let Some(kwargs_dict) = kwargs {
            for (key, val) in kwargs_dict.iter() {
                key_parts.push(format!("{}={}", key, val.repr()?.to_str()?));
            }
        }
        let key = key_parts.join(",");

        let mut cache_lock = cache.lock().unwrap();

        // Check if result is in cache
        if let Some(cached_result) = cache_lock.get(&key) {
            println!("Cache hit for key: {}", key);
            return Ok(cached_result.clone_ref(py));
        }

        // If not, call the function and store the result
        println!("Cache miss for key: {}", key);
        let result = func.bind(py).call(args, kwargs)?;
        let result_unbound = result.unbind();
        cache_lock.insert(key, result_unbound.clone_ref(py));
        Ok(result_unbound)
    };

    let wrapped = PyCFunction::new_closure(py, None, None, wrapper)?;
    Ok(wrapped.into())
}

// 6. Parallel Decorator - Run functions in Rust threads without GIL

/// AsyncHandle - Handle for async operations with pipe communication
#[pyclass]
struct AsyncHandle {
    receiver: Arc<Mutex<Receiver<PyResult<Py<PyAny>>>>>,
    thread_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    is_complete: Arc<Mutex<bool>>,
    result_cache: Arc<Mutex<Option<PyResult<Py<PyAny>>>>>,
    cancel_token: Arc<AtomicBool>,
    func_name: String,
    start_time: Instant,
    task_id: String,
    metadata: Arc<Mutex<HashMap<String, String>>>,
    timeout: Option<f64>,
    on_complete: Arc<Mutex<Option<Py<PyAny>>>>,
    on_error: Arc<Mutex<Option<Py<PyAny>>>>,
    on_progress: Arc<Mutex<Option<Py<PyAny>>>>,
}

#[pymethods]
impl AsyncHandle {
    /// Check if the result is ready (non-blocking)
    fn is_ready(&self) -> PyResult<bool> {
        Ok(*self.is_complete.lock().unwrap())
    }

    /// Try to get the result without blocking (returns None if not ready)
    fn try_get(&self, py: Python) -> PyResult<Option<Py<PyAny>>> {
        // Check cache first
        let mut cache = self.result_cache.lock().unwrap();
        if let Some(ref cached) = *cache {
            return match cached {
                Ok(val) => Ok(Some(val.clone_ref(py))),
                Err(e) => Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                    "Cached error: {}",
                    e
                ))),
            };
        }

        // Try to receive without blocking
        let receiver = self.receiver.lock().unwrap();
        match receiver.try_recv() {
            Ok(result) => {
                *self.is_complete.lock().unwrap() = true;
                match result {
                    Ok(val) => {
                        *cache = Some(Ok(val.clone_ref(py)));
                        Ok(Some(val))
                    }
                    Err(e) => {
                        *cache = Some(Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                            e.to_string(),
                        )));
                        Err(e)
                    }
                }
            }
            Err(_) => Ok(None), // Not ready yet
        }
    }

    /// Get the result (blocking until ready)
    fn get(&self, py: Python) -> PyResult<Py<PyAny>> {
        // Check cache first
        let cache = self.result_cache.lock().unwrap();
        if let Some(ref cached) = *cache {
            return match cached {
                Ok(val) => Ok(val.clone_ref(py)),
                Err(e) => Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                    "Cached error: {}",
                    e
                ))),
            };
        }
        drop(cache); // Release lock before blocking recv

        // CRITICAL: Release GIL before blocking on recv to avoid deadlock
        let result = py
            .detach(|| {
                let receiver = self.receiver.lock().unwrap();
                receiver.recv()
            })
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        *self.is_complete.lock().unwrap() = true;

        // Cache the result
        let mut cache = self.result_cache.lock().unwrap();
        match result {
            Ok(ref val) => {
                *cache = Some(Ok(val.clone_ref(py)));
                Ok(val.clone_ref(py))
            }
            Err(e) => {
                let err_str = e.to_string();
                *cache = Some(Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                    err_str.clone(),
                )));
                Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(err_str))
            }
        }
    }

    /// Wait for completion with timeout (in seconds)
    fn wait(&self, timeout_secs: Option<f64>) -> PyResult<bool> {
        if *self.is_complete.lock().unwrap() {
            return Ok(true);
        }

        if let Some(secs) = timeout_secs {
            thread::sleep(Duration::from_secs_f64(secs));
            Ok(*self.is_complete.lock().unwrap())
        } else {
            // Wait indefinitely by trying to receive
            let _ = self.receiver.lock().unwrap().recv();
            *self.is_complete.lock().unwrap() = true;
            Ok(true)
        }
    }

    /// Cancel the operation (try to join the thread)
    fn cancel(&self) -> PyResult<()> {
        // Set cancellation flag
        self.cancel_token.store(true, Ordering::SeqCst);

        let mut handle = self.thread_handle.lock().unwrap();
        if let Some(h) = handle.take() {
            let _ = h.join();
        }
        Ok(())
    }

    /// Cancel with timeout (in seconds)
    fn cancel_with_timeout(&self, timeout_secs: f64) -> PyResult<bool> {
        self.cancel_token.store(true, Ordering::SeqCst);

        let mut handle = self.thread_handle.lock().unwrap();
        if let Some(h) = handle.take() {
            let start = Instant::now();
            let timeout = Duration::from_secs_f64(timeout_secs);

            // Try to join with timeout
            while start.elapsed() < timeout {
                if h.is_finished() {
                    let _ = h.join();
                    return Ok(true);
                }
                thread::sleep(Duration::from_millis(10));
            }

            return Ok(false); // Timeout
        }
        Ok(true)
    }

    /// Check if task was cancelled
    fn is_cancelled(&self) -> PyResult<bool> {
        Ok(self.cancel_token.load(Ordering::SeqCst))
    }

    /// Get elapsed time since task start (in seconds)
    fn elapsed_time(&self) -> PyResult<f64> {
        Ok(self.start_time.elapsed().as_secs_f64())
    }

    /// Get task name
    fn get_name(&self) -> PyResult<String> {
        Ok(self.func_name.clone())
    }

    /// Get task ID
    fn get_task_id(&self) -> PyResult<String> {
        Ok(self.task_id.clone())
    }

    /// Set metadata
    fn set_metadata(&self, key: String, value: String) -> PyResult<()> {
        self.metadata.lock().unwrap().insert(key, value);
        Ok(())
    }

    /// Get metadata
    fn get_metadata(&self, key: String) -> PyResult<Option<String>> {
        Ok(self.metadata.lock().unwrap().get(&key).cloned())
    }

    /// Get all metadata
    fn get_all_metadata(&self, py: Python) -> PyResult<Py<PyDict>> {
        let dict = PyDict::new(py);
        let metadata = self.metadata.lock().unwrap();
        for (k, v) in metadata.iter() {
            dict.set_item(k, v)?;
        }
        Ok(dict.unbind())
    }

    /// Get timeout value
    fn get_timeout(&self) -> PyResult<Option<f64>> {
        Ok(self.timeout)
    }

    /// Set completion callback
    fn on_complete(&self, callback: Py<PyAny>) -> PyResult<()> {
        *self.on_complete.lock().unwrap() = Some(callback);
        Ok(())
    }

    /// Set error callback
    fn on_error(&self, callback: Py<PyAny>) -> PyResult<()> {
        *self.on_error.lock().unwrap() = Some(callback);
        Ok(())
    }

    /// Set progress callback
    fn on_progress(&self, callback: Py<PyAny>) -> PyResult<()> {
        *self.on_progress.lock().unwrap() = Some(callback);
        Ok(())
    }

    /// Get current progress (0.0 to 1.0)
    fn get_progress(&self) -> PyResult<f64> {
        Ok(TASK_PROGRESS_MAP
            .get(&self.task_id)
            .map(|p| *p)
            .unwrap_or(0.0))
    }
}

/// Parallel function wrapper that returns AsyncHandle
#[pyclass]
struct ParallelWrapper {
    func: Py<PyAny>,
}

#[pymethods]
impl ParallelWrapper {
    #[pyo3(signature = (*args, timeout=None, **kwargs))]
    fn __call__(
        &self,
        py: Python,
        args: &Bound<'_, PyTuple>,
        timeout: Option<f64>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<AsyncHandle>> {
        // Check if shutdown is requested
        if is_shutdown_requested() {
            return Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                "Cannot start new tasks: shutdown in progress"
            ));
        }

        // Wait for available slot (backpressure)
        wait_for_slot();

        // Check memory before starting
        if !check_memory_ok() {
            return Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                "Memory limit reached, cannot start new task"
            ));
        }

        // Clone function reference for the thread
        let func = self.func.clone_ref(py);

        // Generate unique task ID
        let task_id = format!("task_{}", TASK_ID_COUNTER.fetch_add(1, Ordering::SeqCst));
        let task_id_clone = task_id.clone();

        // Register task as active
        register_task(task_id.clone());

        // Get function name for profiling
        let func_name = func
            .bind(py)
            .getattr("__name__")
            .ok()
            .and_then(|n| n.extract::<String>().ok())
            .unwrap_or_else(|| "unknown".to_string());

        // Convert args and kwargs to owned Python objects
        let args_py: Py<PyTuple> = args.clone().unbind();
        let kwargs_py: Option<Py<PyDict>> = kwargs.map(|k| k.clone().unbind());

        // Create channel for communication
        let (sender, receiver): (Sender<PyResult<Py<PyAny>>>, Receiver<PyResult<Py<PyAny>>>) =
            channel();

        let is_complete = Arc::new(Mutex::new(false));
        let is_complete_clone = is_complete.clone();

        let cancel_token = Arc::new(AtomicBool::new(false));
        let cancel_token_clone = cancel_token.clone();

        let func_name_clone = func_name.clone();
        let start_time = Instant::now();

        // Setup timeout if specified
        if let Some(timeout_secs) = timeout {
            let cancel_token_timeout = cancel_token.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_secs_f64(timeout_secs));
                cancel_token_timeout.store(true, Ordering::SeqCst);
            });
        }

        // Spawn Rust thread - release GIL first, then spawn thread
        let handle = py.detach(|| {
            thread::spawn(move || {
                // Acquire GIL inside the thread to call Python function
                Python::attach(|py| {
                    let exec_start = Instant::now();

                    // Check shutdown or cancellation before execution
                    if is_shutdown_requested() || cancel_token_clone.load(Ordering::SeqCst) {
                        let reason = if is_shutdown_requested() {
                            "Task cancelled: shutdown requested"
                        } else {
                            "Task was cancelled or timed out"
                        };

                        let task_error = TaskError {
                            task_name: func_name_clone.clone(),
                            elapsed_time: exec_start.elapsed().as_secs_f64(),
                            error_message: reason.to_string(),
                            error_type: "CancellationError".to_string(),
                            task_id: task_id_clone.clone(),
                        };

                        let _ = sender.send(Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                            task_error.__str__()
                        )));
                        *is_complete_clone.lock().unwrap() = true;
                        unregister_task(&task_id_clone);
                        return;
                    }

                    let result = func
                        .bind(py)
                        .call(args_py.bind(py), kwargs_py.as_ref().map(|k| k.bind(py)));

                    let exec_time = exec_start.elapsed().as_secs_f64() * 1000.0; // Convert to ms

                    let to_send = match result {
                        Ok(val) => {
                            record_task_execution(&func_name_clone, exec_time, true);
                            Ok(val.unbind())
                        }
                        Err(e) => {
                            record_task_execution(&func_name_clone, exec_time, false);

                            // Create enhanced error with context
                            let error_type = e.get_type(py).name()
                                .map(|n| n.to_string())
                                .unwrap_or_else(|_| "UnknownError".to_string());

                            let task_error = TaskError {
                                task_name: func_name_clone.clone(),
                                elapsed_time: exec_start.elapsed().as_secs_f64(),
                                error_message: e.to_string(),
                                error_type,
                                task_id: task_id_clone.clone(),
                            };

                            Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                                task_error.__str__()
                            ))
                        }
                    };

                    // Send result through channel
                    let _ = sender.send(to_send);
                    *is_complete_clone.lock().unwrap() = true;

                    // Unregister task
                    unregister_task(&task_id_clone);
                });
            })
        });

        // Create AsyncHandle
        let async_handle = AsyncHandle {
            receiver: Arc::new(Mutex::new(receiver)),
            thread_handle: Arc::new(Mutex::new(Some(handle))),
            is_complete,
            result_cache: Arc::new(Mutex::new(None)),
            cancel_token,
            func_name,
            start_time,
            task_id,
            metadata: Arc::new(Mutex::new(HashMap::new())),
            timeout,
            on_complete: Arc::new(Mutex::new(None)),
            on_error: Arc::new(Mutex::new(None)),
            on_progress: Arc::new(Mutex::new(None)),
        };

        Py::new(py, async_handle)
    }

    fn __get__(
        slf: PyRef<'_, Self>,
        obj: &Bound<'_, PyAny>,
        _objtype: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let py = slf.py();

        if obj.is_none() {
            // Unbound method access - return self
            return Ok(slf.into_bound_py_any(py)?.unbind());
        }

        // Bound method access - create a new ParallelWrapper with bound function
        let functools = py.import("functools")?;
        let partial = functools.getattr("partial")?;
        let bound_func = partial.call1((slf.func.bind(py), obj))?.unbind();

        Py::new(py, ParallelWrapper { func: bound_func }).map(|p| p.into())
    }
}

/// Decorator to run functions in parallel Rust threads without GIL
#[pyfunction]
fn parallel(py: Python, func: Py<PyAny>) -> PyResult<Py<ParallelWrapper>> {
    Py::new(py, ParallelWrapper { func })
}

// =============================================================================
// OPTIMIZED IMPLEMENTATIONS
// =============================================================================

/// Optimized AsyncHandle using crossbeam channels (lock-free, better performance)
#[pyclass]
struct AsyncHandleFast {
    receiver: Arc<Mutex<CrossbeamReceiver<PyResult<Py<PyAny>>>>>,
    is_complete: Arc<Mutex<bool>>,
    result_cache: Arc<Mutex<Option<PyResult<Py<PyAny>>>>>,
}

#[pymethods]
impl AsyncHandleFast {
    fn is_ready(&self) -> PyResult<bool> {
        Ok(*self.is_complete.lock().unwrap())
    }

    fn try_get(&self, py: Python) -> PyResult<Option<Py<PyAny>>> {
        let mut cache = self.result_cache.lock().unwrap();
        if let Some(ref cached) = *cache {
            return match cached {
                Ok(val) => Ok(Some(val.clone_ref(py))),
                Err(e) => Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                    "Cached error: {}",
                    e
                ))),
            };
        }

        let receiver = self.receiver.lock().unwrap();
        match receiver.try_recv() {
            Ok(result) => {
                *self.is_complete.lock().unwrap() = true;
                match result {
                    Ok(val) => {
                        *cache = Some(Ok(val.clone_ref(py)));
                        Ok(Some(val))
                    }
                    Err(e) => {
                        *cache = Some(Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                            e.to_string(),
                        )));
                        Err(e)
                    }
                }
            }
            Err(_) => Ok(None),
        }
    }

    fn get(&self, py: Python) -> PyResult<Py<PyAny>> {
        let cache = self.result_cache.lock().unwrap();
        if let Some(ref cached) = *cache {
            return match cached {
                Ok(val) => Ok(val.clone_ref(py)),
                Err(e) => Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                    "Cached error: {}",
                    e
                ))),
            };
        }
        drop(cache);

        // Release GIL before blocking
        let result = py
            .detach(|| {
                let receiver = self.receiver.lock().unwrap();
                receiver.recv()
            })
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        *self.is_complete.lock().unwrap() = true;

        let mut cache = self.result_cache.lock().unwrap();
        match result {
            Ok(ref val) => {
                *cache = Some(Ok(val.clone_ref(py)));
                Ok(val.clone_ref(py))
            }
            Err(e) => {
                let err_str = e.to_string();
                *cache = Some(Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                    err_str.clone(),
                )));
                Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(err_str))
            }
        }
    }
}

/// Optimized parallel wrapper using crossbeam channels
#[pyclass]
struct ParallelFastWrapper {
    func: Py<PyAny>,
}

#[pymethods]
impl ParallelFastWrapper {
    #[pyo3(signature = (*args, **kwargs))]
    fn __call__(
        &self,
        py: Python,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<AsyncHandleFast>> {
        let func = self.func.clone_ref(py);
        let args_py: Py<PyTuple> = args.clone().unbind();
        let kwargs_py: Option<Py<PyDict>> = kwargs.map(|k| k.clone().unbind());

        // Use crossbeam unbounded channel for better performance
        let (sender, receiver): (
            CrossbeamSender<PyResult<Py<PyAny>>>,
            CrossbeamReceiver<PyResult<Py<PyAny>>>,
        ) = unbounded();

        let is_complete = Arc::new(Mutex::new(false));
        let is_complete_clone = is_complete.clone();

        // Spawn thread without GIL
        py.detach(|| {
            thread::spawn(move || {
                Python::attach(|py| {
                    let result = func
                        .bind(py)
                        .call(args_py.bind(py), kwargs_py.as_ref().map(|k| k.bind(py)));

                    let to_send = match result {
                        Ok(val) => Ok(val.unbind()),
                        Err(e) => Err(e),
                    };

                    let _ = sender.send(to_send);
                    *is_complete_clone.lock().unwrap() = true;
                });
            })
        });

        let async_handle = AsyncHandleFast {
            receiver: Arc::new(Mutex::new(receiver)),
            is_complete,
            result_cache: Arc::new(Mutex::new(None)),
        };

        Py::new(py, async_handle)
    }
}

/// Optimized parallel decorator using crossbeam channels
#[pyfunction]
fn parallel_fast(py: Python, func: Py<PyAny>) -> PyResult<Py<ParallelFastWrapper>> {
    Py::new(py, ParallelFastWrapper { func })
}

/// Thread pool using rayon for better resource management
#[pyclass]
struct ParallelPoolWrapper {
    func: Py<PyAny>,
}

#[pymethods]
impl ParallelPoolWrapper {
    #[pyo3(signature = (*args, **kwargs))]
    fn __call__(
        &self,
        py: Python,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<AsyncHandleFast>> {
        let func = self.func.clone_ref(py);
        let args_py: Py<PyTuple> = args.clone().unbind();
        let kwargs_py: Option<Py<PyDict>> = kwargs.map(|k| k.clone().unbind());

        let (sender, receiver) = unbounded();
        let is_complete = Arc::new(Mutex::new(false));
        let is_complete_clone = is_complete.clone();

        // Use rayon thread pool - better resource management
        py.detach(|| {
            rayon::spawn(move || {
                Python::attach(|py| {
                    let result = func
                        .bind(py)
                        .call(args_py.bind(py), kwargs_py.as_ref().map(|k| k.bind(py)));

                    let to_send = match result {
                        Ok(val) => Ok(val.unbind()),
                        Err(e) => Err(e),
                    };

                    let _ = sender.send(to_send);
                    *is_complete_clone.lock().unwrap() = true;
                });
            });
        });

        let async_handle = AsyncHandleFast {
            receiver: Arc::new(Mutex::new(receiver)),
            is_complete,
            result_cache: Arc::new(Mutex::new(None)),
        };

        Py::new(py, async_handle)
    }
}

/// Parallel decorator using rayon thread pool (optimized for many small tasks)
#[pyfunction]
fn parallel_pool(py: Python, func: Py<PyAny>) -> PyResult<Py<ParallelPoolWrapper>> {
    Py::new(py, ParallelPoolWrapper { func })
}

/// Optimized memoize using DashMap (lock-free concurrent hashmap)
#[pyfunction]
fn memoize_fast(py: Python, func: Py<PyAny>) -> PyResult<Py<PyAny>> {
    // Use DashMap - lock-free concurrent hashmap
    let cache: Arc<DashMap<String, Py<PyAny>>> = Arc::new(DashMap::new());
    let func_clone = func.clone_ref(py);

    let wrapper = move |args: &Bound<'_, PyTuple>,
                        kwargs: Option<&Bound<'_, PyDict>>|
          -> PyResult<Py<PyAny>> {
        let py = args.py();

        // Create cache key
        let mut key_parts: Vec<String> = vec![];
        for arg in args.iter() {
            key_parts.push(arg.repr()?.to_str()?.to_string());
        }
        if let Some(kwargs_dict) = kwargs {
            for (key, val) in kwargs_dict.iter() {
                key_parts.push(format!("{}={}", key, val.repr()?.to_str()?));
            }
        }
        let key = key_parts.join(",");

        // Check cache (lock-free read)
        if let Some(cached) = cache.get(&key) {
            println!("Cache hit for key: {}", key);
            return Ok(cached.clone_ref(py));
        }

        // Cache miss - compute result
        println!("Cache miss for key: {}", key);
        let result = func_clone.bind(py).call(args, kwargs)?;
        let result_unbound = result.unbind();

        // Insert into cache (lock-free write)
        cache.insert(key, result_unbound.clone_ref(py));

        Ok(result_unbound)
    };

    let wrapped = PyCFunction::new_closure(py, None, None, wrapper)?;

    let method_wrapper = Py::new(
        py,
        MethodWrapper {
            func: func.clone_ref(py),
            wrapper: wrapped.into(),
        },
    )?;
    Ok(method_wrapper.into())
}

/// Batch parallel processing - execute multiple functions in parallel
#[pyfunction]
fn parallel_map(py: Python, func: Py<PyAny>, items: Vec<Py<PyAny>>) -> PyResult<Vec<Py<PyAny>>> {
    py.detach(|| {
        // Use rayon for parallel iteration
        let results: Vec<_> = items
            .par_iter()
            .map(|item| {
                Python::attach(|py| func.bind(py).call1((item.bind(py),)).map(|r| r.unbind()))
            })
            .collect();

        // Convert results
        results.into_iter().collect()
    })
}

/// Priority parallel wrapper - tasks execute based on priority
#[pyclass]
struct PriorityParallelWrapper {
    func: Py<PyAny>,
}

#[pymethods]
impl PriorityParallelWrapper {
    #[pyo3(signature = (*args, priority=0, **kwargs))]
    fn __call__(
        &self,
        py: Python,
        args: &Bound<'_, PyTuple>,
        priority: i32,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<AsyncHandleFast>> {
        let func = self.func.clone_ref(py);
        let args_py: Py<PyTuple> = args.clone().unbind();
        let kwargs_py: Option<Py<PyDict>> = kwargs.map(|k| k.clone().unbind());

        let (sender, receiver) = unbounded();
        let is_complete = Arc::new(Mutex::new(false));

        // Create priority task
        let task = PriorityTask {
            priority,
            func,
            args: args_py,
            kwargs: kwargs_py,
            sender,
        };

        // Push to priority queue
        PRIORITY_QUEUE.lock().unwrap().push(task);

        // Ensure worker is running
        if !PRIORITY_WORKER_RUNNING.load(Ordering::SeqCst) {
            start_priority_worker(py)?;
        }

        let async_handle = AsyncHandleFast {
            receiver: Arc::new(Mutex::new(receiver)),
            is_complete,
            result_cache: Arc::new(Mutex::new(None)),
        };

        Py::new(py, async_handle)
    }
}

/// Priority parallel decorator
#[pyfunction]
fn parallel_priority(py: Python, func: Py<PyAny>) -> PyResult<Py<PriorityParallelWrapper>> {
    Py::new(py, PriorityParallelWrapper { func })
}

/// Decorator with profiling enabled
#[pyfunction]
fn profiled(py: Python, func: Py<PyAny>) -> PyResult<Py<PyAny>> {
    let func_clone = func.clone_ref(py);
    let wrapper = move |args: &Bound<'_, PyTuple>,
                        kwargs: Option<&Bound<'_, PyDict>>|
          -> PyResult<Py<PyAny>> {
        let py = args.py();

        let func_name = func_clone
            .bind(py)
            .getattr("__name__")
            .ok()
            .and_then(|n| n.extract::<String>().ok())
            .unwrap_or_else(|| "unknown".to_string());

        let start = Instant::now();
        let result = func_clone.bind(py).call(args, kwargs);
        let duration_ms = start.elapsed().as_secs_f64() * 1000.0;

        match result {
            Ok(val) => {
                record_task_execution(&func_name, duration_ms, true);
                Ok(val.unbind())
            }
            Err(e) => {
                record_task_execution(&func_name, duration_ms, false);
                Err(e)
            }
        }
    };

    let wrapped = PyCFunction::new_closure(py, None, None, wrapper)?;

    let method_wrapper = Py::new(
        py,
        MethodWrapper {
            func: func.clone_ref(py),
            wrapper: wrapped.into(),
        },
    )?;
    Ok(method_wrapper.into())
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Gather results from multiple handles
#[pyfunction]
#[pyo3(signature = (handles, on_error="raise"))]
fn gather(py: Python, handles: Vec<Py<AsyncHandle>>, on_error: &str) -> PyResult<Vec<Py<PyAny>>> {
    let mut results = Vec::new();

    for handle in handles {
        let h = handle.bind(py);
        match h.call_method0("get") {
            Ok(result) => results.push(result.unbind()),
            Err(e) => match on_error {
                "raise" => return Err(e),
                "skip" => continue,
                "none" => results.push(py.None()),
                _ => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                    "on_error must be 'raise', 'skip', or 'none'"
                )),
            },
        }
    }

    Ok(results)
}

/// Context manager for parallel execution
#[pyclass]
struct ParallelContext {
    handles: Arc<Mutex<Vec<Py<AsyncHandle>>>>,
    timeout: Option<f64>,
}

#[pymethods]
impl ParallelContext {
    #[new]
    #[pyo3(signature = (timeout=None))]
    fn new(timeout: Option<f64>) -> Self {
        ParallelContext {
            handles: Arc::new(Mutex::new(Vec::new())),
            timeout,
        }
    }

    /// Submit a task
    fn submit(&self, py: Python, func: Py<PyAny>, args: &Bound<'_, PyTuple>) -> PyResult<Py<AsyncHandle>> {
        // Call the function with timeout if specified
        let handle = if let Some(timeout) = self.timeout {
            func.bind(py).call_method1("__call__", (args, ("timeout", timeout)))?
        } else {
            func.bind(py).call(args, None)?
        };

        let async_handle: Py<AsyncHandle> = handle.extract()?;
        self.handles.lock().unwrap().push(async_handle.clone_ref(py));
        Ok(async_handle)
    }

    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __exit__(
        &self,
        py: Python,
        _exc_type: &Bound<'_, PyAny>,
        _exc_val: &Bound<'_, PyAny>,
        _exc_tb: &Bound<'_, PyAny>,
    ) -> PyResult<bool> {
        // Wait for all tasks
        let handles_guard = self.handles.lock().unwrap();
        for handle in handles_guard.iter() {
            let _ = handle.bind(py).call_method0("get");
        }
        Ok(false)
    }
}

/// Enhanced retry with exponential backoff
#[pyfunction]
#[pyo3(signature = (*, max_attempts=3, backoff="exponential", initial_delay=1.0, max_delay=60.0))]
fn retry_backoff(
    _py: Python<'_>,
    max_attempts: usize,
    backoff: &str,
    initial_delay: f64,
    max_delay: f64,
) -> PyResult<Py<PyAny>> {
    let backoff_owned = backoff.to_string();
    let factory = move |py: Python<'_>, func: Py<PyAny>| -> PyResult<Py<PyAny>> {
        let backoff_clone = backoff_owned.clone();
        let wrapper = move |args: &Bound<'_, PyTuple>,
                            kwargs: Option<&Bound<'_, PyDict>>|
              -> PyResult<Py<PyAny>> {
            let py = args.py();
            let mut last_err = None;
            let mut delay = initial_delay;

            for attempt in 0..max_attempts {
                match func.bind(py).call(args, kwargs) {
                    Ok(res) => return Ok(res.unbind()),
                    Err(e) => {
                        println!("Attempt {}/{} failed: {:?}", attempt + 1, max_attempts, e.to_string());
                        last_err = Some(e);

                        if attempt < max_attempts - 1 {
                            thread::sleep(Duration::from_secs_f64(delay));

                            // Calculate next delay
                            delay = match backoff_clone.as_str() {
                                "exponential" => (delay * 2.0).min(max_delay),
                                "linear" => (delay + initial_delay).min(max_delay),
                                _ => delay,
                            };
                        }
                    }
                }
            }
            Err(last_err.unwrap())
        };
        let wrapped = PyCFunction::new_closure(py, None, None, wrapper)?;
        Ok(wrapped.into())
    };

    let decorator = PyCFunction::new_closure(
        _py,
        None,
        None,
        move |args: &Bound<'_, PyTuple>, _kwargs: Option<&Bound<'_, PyDict>>| {
            let func = args.get_item(0)?.unbind();
            factory(args.py(), func)
        },
    )?;
    Ok(decorator.into())
}

/// This module is implemented in Rust.
#[pymodule]
fn makeParallel(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Original decorators
    m.add_function(wrap_pyfunction!(timer, m)?)?;
    m.add_function(wrap_pyfunction!(log_calls, m)?)?;
    m.add_class::<CallCounter>()?;
    m.add_function(wrap_pyfunction!(retry, m)?)?;
    m.add_function(wrap_pyfunction!(memoize, m)?)?;
    m.add_function(wrap_pyfunction!(parallel, m)?)?;
    m.add_class::<AsyncHandle>()?;

    // Optimized versions
    m.add_function(wrap_pyfunction!(parallel_fast, m)?)?;
    m.add_function(wrap_pyfunction!(parallel_pool, m)?)?;
    m.add_function(wrap_pyfunction!(memoize_fast, m)?)?;
    m.add_function(wrap_pyfunction!(parallel_map, m)?)?;
    m.add_class::<AsyncHandleFast>()?;

    // Thread pool configuration
    m.add_function(wrap_pyfunction!(configure_thread_pool, m)?)?;
    m.add_function(wrap_pyfunction!(get_thread_pool_info, m)?)?;

    // Priority queue
    m.add_function(wrap_pyfunction!(parallel_priority, m)?)?;
    m.add_function(wrap_pyfunction!(start_priority_worker, m)?)?;
    m.add_function(wrap_pyfunction!(stop_priority_worker, m)?)?;

    // Performance profiling
    m.add_function(wrap_pyfunction!(profiled, m)?)?;
    m.add_function(wrap_pyfunction!(get_metrics, m)?)?;
    m.add_function(wrap_pyfunction!(get_all_metrics, m)?)?;
    m.add_function(wrap_pyfunction!(reset_metrics, m)?)?;
    m.add_class::<PerformanceMetrics>()?;

    // Error handling and shutdown
    m.add_class::<TaskError>()?;
    m.add_function(wrap_pyfunction!(shutdown, m)?)?;
    m.add_function(wrap_pyfunction!(reset_shutdown, m)?)?;
    m.add_function(wrap_pyfunction!(get_active_task_count, m)?)?;

    // Backpressure and resource management
    m.add_function(wrap_pyfunction!(set_max_concurrent_tasks, m)?)?;
    m.add_function(wrap_pyfunction!(configure_memory_limit, m)?)?;

    // Progress tracking
    m.add_function(wrap_pyfunction!(report_progress, m)?)?;

    // Helper functions
    m.add_function(wrap_pyfunction!(gather, m)?)?;
    m.add_class::<ParallelContext>()?;
    m.add_function(wrap_pyfunction!(retry_backoff, m)?)?;

    Ok(())
}
