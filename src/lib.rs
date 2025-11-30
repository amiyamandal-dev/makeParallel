use pyo3::IntoPyObjectExt;
use pyo3::prelude::*;
use pyo3::types::{PyCFunction, PyDict, PyTuple};
use pyo3::wrap_pyfunction;
use std::collections::{BinaryHeap, HashMap};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use std::cmp::Ordering as CmpOrdering;
use std::cell::RefCell;

// Optimized imports
use crossbeam::channel::{Receiver as CrossbeamReceiver, Sender as CrossbeamSender, unbounded};
use dashmap::DashMap;
use rayon::prelude::*;
use once_cell::sync::Lazy;
use parking_lot::Mutex;  // Faster mutex implementation

// Logging
use log::{debug, warn, error};

// System monitoring
use sysinfo::System;

// Module imports
mod types;
use types::TaskError as CustomTaskError;

type TaskError = CustomTaskError;

// Callback types
type CallbackFunc = Arc<Mutex<Option<Py<PyAny>>>>;

// Task dependency tracking
static TASK_DEPENDENCIES: Lazy<Arc<DashMap<String, Vec<String>>>> =
    Lazy::new(|| Arc::new(DashMap::new()));

static TASK_RESULTS: Lazy<Arc<DashMap<String, Py<PyAny>>>> =
    Lazy::new(|| Arc::new(DashMap::new()));

// Store task errors for dependency failure propagation
static TASK_ERRORS: Lazy<Arc<DashMap<String, String>>> =
    Lazy::new(|| Arc::new(DashMap::new()));

// Track dependency reference counts for cleanup
static DEPENDENCY_COUNTS: Lazy<Arc<DashMap<String, usize>>> =
    Lazy::new(|| Arc::new(DashMap::new()));

// Timeout cancellation handles
static TIMEOUT_HANDLES: Lazy<Arc<Mutex<Vec<(String, Sender<()>)>>>> =
    Lazy::new(|| Arc::new(Mutex::new(Vec::new())));

// System monitor for memory checking
static SYSTEM_MONITOR: Lazy<Mutex<System>> = Lazy::new(|| Mutex::new(System::new_all()));

/// Global shutdown flag
static SHUTDOWN_FLAG: Lazy<Arc<AtomicBool>> = Lazy::new(|| Arc::new(AtomicBool::new(false)));

/// Active task handles for shutdown
static ACTIVE_TASKS: Lazy<Arc<Mutex<Vec<String>>>> = Lazy::new(|| Arc::new(Mutex::new(Vec::new())));

/// Task ID counter
static TASK_ID_COUNTER: Lazy<Arc<AtomicU64>> = Lazy::new(|| Arc::new(AtomicU64::new(0)));

/// Check if shutdown is requested
fn is_shutdown_requested() -> bool {
    SHUTDOWN_FLAG.load(Ordering::Acquire)
}

/// Register a task as active
fn register_task(task_id: String) {
    ACTIVE_TASKS.lock().push(task_id);
}

/// Unregister a task
fn unregister_task(task_id: &str) {
    let mut tasks = ACTIVE_TASKS.lock();
    tasks.retain(|id| id != task_id);
}

/// Get active task count
#[pyfunction]
fn get_active_task_count() -> usize {
    ACTIVE_TASKS.lock().len()
}

/// Initiate graceful shutdown
#[pyfunction]
fn shutdown(timeout_secs: Option<f64>, cancel_pending: bool) -> PyResult<bool> {
    println!("Initiating graceful shutdown...");
    SHUTDOWN_FLAG.store(true, Ordering::Release);

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
    SHUTDOWN_FLAG.store(false, Ordering::Release);
    Ok(())
}

/// Global concurrent task limit
static MAX_CONCURRENT_TASKS: Lazy<Arc<Mutex<Option<usize>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

/// Set maximum concurrent tasks
#[pyfunction]
fn set_max_concurrent_tasks(max_tasks: usize) -> PyResult<()> {
    *MAX_CONCURRENT_TASKS.lock() = Some(max_tasks);
    Ok(())
}

/// Wait for available slot (backpressure)
fn wait_for_slot() {
    if let Some(max) = *MAX_CONCURRENT_TASKS.lock() {
        let start = Instant::now();
        let timeout = Duration::from_secs(300); // 5 minute timeout
        let mut backoff = Duration::from_millis(10);

        while get_active_task_count() >= max {
            // CRITICAL FIX: Check shutdown
            if is_shutdown_requested() {
                warn!("wait_for_slot cancelled: shutdown in progress");
                return;
            }

            // CRITICAL FIX: Add timeout
            if start.elapsed() > timeout {
                error!("wait_for_slot timed out after 5 minutes");
                return;
            }

            thread::sleep(backoff);

            // CRITICAL FIX: Exponential backoff
            backoff = (backoff * 2).min(Duration::from_secs(1));
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
    *MEMORY_LIMIT_PERCENT.lock() = Some(max_memory_percent);
    Ok(())
}

/// Check if memory usage is acceptable
fn check_memory_ok() -> bool {
    if let Some(limit_percent) = *MEMORY_LIMIT_PERCENT.lock() {
        // CRITICAL FIX: Implement actual memory monitoring
        let mut sys = SYSTEM_MONITOR.lock();
        sys.refresh_memory();

        let total = sys.total_memory();
        let used = sys.used_memory();
        let usage_percent = (used as f64 / total as f64) * 100.0;

        if usage_percent > limit_percent {
            warn!(
                "Memory limit exceeded: {:.1}% used (limit: {:.1}%)",
                usage_percent,
                limit_percent
            );
            return false;
        }

        debug!("Memory usage: {:.1}%", usage_percent);
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

// Thread-local storage for current task ID
thread_local! {
    static CURRENT_TASK_ID: RefCell<Option<String>> = RefCell::new(None);
}

/// Set the current task ID for this thread (internal use)
fn set_current_task_id(task_id: Option<String>) {
    CURRENT_TASK_ID.with(|id| {
        *id.borrow_mut() = task_id;
    });
}

/// Get the current task ID for this thread
#[pyfunction]
fn get_current_task_id() -> PyResult<Option<String>> {
    Ok(CURRENT_TASK_ID.with(|id| id.borrow().clone()))
}

/// Report progress from within a task (with explicit task_id)
#[pyfunction]
#[pyo3(signature = (progress, task_id=None))]
fn report_progress(progress: f64, task_id: Option<String>) -> PyResult<()> {
    // CRITICAL FIX: Add NaN/Inf check
    if !progress.is_finite() {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            "progress must be a finite number (not NaN or Infinity)"
        ));
    }

    if progress < 0.0 || progress > 1.0 {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            "progress must be between 0.0 and 1.0"
        ));
    }

    // Use provided task_id or get from thread-local storage
    let actual_task_id = if let Some(tid) = task_id {
        tid
    } else {
        CURRENT_TASK_ID.with(|id| {
            id.borrow().clone().ok_or_else(|| {
                PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                    "No task_id found. report_progress must be called from within a @parallel decorated function, or you must provide task_id explicitly."
                )
            })
        })?
    };

    TASK_PROGRESS_MAP.insert(actual_task_id.clone(), progress);

    // CRITICAL FIX: Non-blocking callback with error handling
    if let Some(callback) = TASK_PROGRESS_CALLBACKS.get(&actual_task_id) {
        Python::attach(|py| {
            // Execute callback with error handling
            match callback.bind(py).call1((progress,)) {
                Ok(_) => {},
                Err(e) => {
                    warn!("Progress callback failed for task {}: {}", actual_task_id, e);
                }
            }
        });
    }

    Ok(())
}

/// Global map for progress callbacks
static TASK_PROGRESS_CALLBACKS: Lazy<Arc<DashMap<String, Py<PyAny>>>> =
    Lazy::new(|| Arc::new(DashMap::new()));

/// Register progress callback for a task (internal)
fn register_progress_callback(task_id: String, callback: Py<PyAny>) {
    TASK_PROGRESS_CALLBACKS.insert(task_id, callback);
}

/// Unregister progress callback (internal)
fn unregister_progress_callback(task_id: &str) {
    TASK_PROGRESS_CALLBACKS.remove(task_id);
}

/// Clear progress for a completed task (internal cleanup)
fn clear_task_progress(task_id: &str) {
    TASK_PROGRESS_MAP.remove(task_id);
    unregister_progress_callback(task_id);
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

        *CUSTOM_THREAD_POOL.lock() = Some(pool);
        Ok(())
    })
}

/// Get current thread pool info
#[pyfunction]
fn get_thread_pool_info(py: Python) -> PyResult<Py<PyDict>> {
    let dict = PyDict::new(py);
    let pool = CUSTOM_THREAD_POOL.lock();

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
    if PRIORITY_WORKER_RUNNING.load(Ordering::Acquire) {
        return Ok(());
    }

    PRIORITY_WORKER_RUNNING.store(true, Ordering::Release);

    py.detach(|| {
        thread::spawn(move || {
            while PRIORITY_WORKER_RUNNING.load(Ordering::Acquire) {
                let task_opt = {
                    let mut queue = PRIORITY_QUEUE.lock();
                    queue.pop()
                };

                if let Some(task) = task_opt {
                    Python::attach(|py| {
                        let exec_start = Instant::now();

                        // Get function name for profiling
                        let func_name = task.func
                            .bind(py)
                            .getattr("__name__")
                            .ok()
                            .and_then(|n| n.extract::<String>().ok())
                            .unwrap_or_else(|| "unknown".to_string());

                        let result = task.func
                            .bind(py)
                            .call(task.args.bind(py), task.kwargs.as_ref().map(|k| k.bind(py)));

                        let exec_time = exec_start.elapsed().as_secs_f64() * 1000.0; // Convert to ms

                        let to_send = match result {
                            Ok(val) => {
                                record_task_execution(&func_name, exec_time, true);
                                Ok(val.unbind())
                            }
                            Err(e) => {
                                record_task_execution(&func_name, exec_time, false);
                                Err(e)
                            }
                        };

                        // CRITICAL FIX: Handle channel send errors
                        if let Err(e) = task.sender.send(to_send) {
                            error!("Failed to send priority task result: {}", e);
                        }
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
    PRIORITY_WORKER_RUNNING.store(false, Ordering::Release);
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
    TASK_COUNTER.fetch_add(1, Ordering::Relaxed);

    if success {
        COMPLETED_COUNTER.fetch_add(1, Ordering::Relaxed);
    } else {
        FAILED_COUNTER.fetch_add(1, Ordering::Relaxed);
    }

    let mut metrics = METRICS.lock();
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
    let metrics = METRICS.lock();
    Ok(metrics.get(&name).cloned())
}

/// Get all performance metrics
#[pyfunction]
fn get_all_metrics(py: Python) -> PyResult<Py<PyDict>> {
    let dict = PyDict::new(py);
    let metrics = METRICS.lock();

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
    METRICS.lock().clear();
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
        let mut count = self.call_count.lock();
        *count += 1;
        Ok(self.func.bind(py).call(args, kwargs)?.unbind())
    }

    #[getter]
    fn get_call_count(&self) -> PyResult<i32> {
        Ok(*self.call_count.lock())
    }

    fn reset(&self) -> PyResult<()> {
        *self.call_count.lock() = 0;
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
        Ok(*self.call_count.lock())
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

        let mut cache_lock = cache.lock();

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
        Ok(*self.is_complete.lock())
    }

    /// Try to get the result without blocking (returns None if not ready)
    fn try_get(&self, py: Python) -> PyResult<Option<Py<PyAny>>> {
        // Check cache first
        let mut cache = self.result_cache.lock();
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
        let receiver = self.receiver.lock();
        match receiver.try_recv() {
            Ok(result) => {
                *self.is_complete.lock() = true;
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
        let cache = self.result_cache.lock();
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
                let receiver = self.receiver.lock();
                receiver.recv()
            })
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        *self.is_complete.lock() = true;

        // Cache the result and trigger callbacks
        let mut cache = self.result_cache.lock();
        match result {
            Ok(ref val) => {
                *cache = Some(Ok(val.clone_ref(py)));

                // CRITICAL FIX: Proper callback error handling
                if let Some(ref callback) = *self.on_complete.lock() {
                    match callback.bind(py).call1((val.bind(py),)) {
                        Ok(_) => {},
                        Err(e) => {
                            error!("on_complete callback failed: {}", e);
                            // Don't propagate callback errors to task result
                        }
                    }
                }

                Ok(val.clone_ref(py))
            }
            Err(e) => {
                let err_str = e.to_string();
                *cache = Some(Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                    err_str.clone(),
                )));

                // CRITICAL FIX: Proper error callback handling
                if let Some(ref callback) = *self.on_error.lock() {
                    match callback.bind(py).call1((err_str.clone(),)) {
                        Ok(_) => {},
                        Err(e) => {
                            error!("on_error callback failed: {}", e);
                        }
                    }
                }

                Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(err_str))
            }
        }
    }

    /// Wait for completion with timeout (in seconds)
    fn wait(&self, timeout_secs: Option<f64>) -> PyResult<bool> {
        if *self.is_complete.lock() {
            return Ok(true);
        }

        if let Some(secs) = timeout_secs {
            thread::sleep(Duration::from_secs_f64(secs));
            Ok(*self.is_complete.lock())
        } else {
            // Wait indefinitely by trying to receive
            let _ = self.receiver.lock().recv();
            *self.is_complete.lock() = true;
            Ok(true)
        }
    }

    /// Cancel the operation (non-blocking - just sets the flag)
    fn cancel(&self) -> PyResult<()> {
        // Set cancellation flag with Release ordering
        self.cancel_token.store(true, Ordering::Release);

        // Mark as complete to prevent further waits
        *self.is_complete.lock() = true;

        // Don't join the thread - that would block!
        // The thread will check the flag and exit on its own
        Ok(())
    }

    /// Cancel with timeout (in seconds)
    fn cancel_with_timeout(&self, timeout_secs: f64) -> PyResult<bool> {
        self.cancel_token.store(true, Ordering::Release);

        let mut handle = self.thread_handle.lock();
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
        Ok(self.cancel_token.load(Ordering::Acquire))
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
        self.metadata.lock().insert(key, value);
        Ok(())
    }

    /// Get metadata
    fn get_metadata(&self, key: String) -> PyResult<Option<String>> {
        Ok(self.metadata.lock().get(&key).cloned())
    }

    /// Get all metadata
    fn get_all_metadata(&self, py: Python) -> PyResult<Py<PyDict>> {
        let dict = PyDict::new(py);
        let metadata = self.metadata.lock();
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
        *self.on_complete.lock() = Some(callback);
        Ok(())
    }

    /// Set error callback
    fn on_error(&self, callback: Py<PyAny>) -> PyResult<()> {
        *self.on_error.lock() = Some(callback);
        Ok(())
    }

    /// Set progress callback
    fn on_progress(&self, py: Python, callback: Py<PyAny>) -> PyResult<()> {
        *self.on_progress.lock() = Some(callback.clone_ref(py));
        register_progress_callback(self.task_id.clone(), callback);
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
        let task_id = format!("task_{}", TASK_ID_COUNTER.fetch_add(1, Ordering::Relaxed));
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
                cancel_token_timeout.store(true, Ordering::Release);
            });
        }

        // Spawn Rust thread - release GIL first, then spawn thread
        let handle = py.detach(|| {
            thread::spawn(move || {
                // Acquire GIL inside the thread to call Python function
                Python::attach(|py| {
                    let exec_start = Instant::now();

                    // Set task_id in thread-local storage for progress reporting
                    set_current_task_id(Some(task_id_clone.clone()));

                    // Check shutdown or cancellation before execution
                    if is_shutdown_requested() || cancel_token_clone.load(Ordering::Acquire) {
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

                        // CRITICAL FIX: Handle channel send errors
                        if let Err(e) = sender.send(Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                            task_error.__str__()
                        ))) {
                            error!("Failed to send cancellation error for task {}: {}", task_id_clone, e);
                            store_task_error(task_id_clone.clone(), format!("Cancellation failed: {}", e));
                        }
                        *is_complete_clone.lock() = true;
                        unregister_task(&task_id_clone);
                        clear_task_progress(&task_id_clone);
                        set_current_task_id(None);
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

                    // CRITICAL FIX: Handle channel send errors
                    if let Err(e) = sender.send(to_send) {
                        error!("Failed to send task result for task {}: {}", task_id_clone, e);
                        store_task_error(task_id_clone.clone(), format!("Channel send failed: {}", e));
                    }
                    *is_complete_clone.lock() = true;

                    // Cleanup: unregister task and clear progress
                    unregister_task(&task_id_clone);
                    clear_task_progress(&task_id_clone);
                    set_current_task_id(None);
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
        Ok(*self.is_complete.lock())
    }

    fn try_get(&self, py: Python) -> PyResult<Option<Py<PyAny>>> {
        let mut cache = self.result_cache.lock();
        if let Some(ref cached) = *cache {
            return match cached {
                Ok(val) => Ok(Some(val.clone_ref(py))),
                Err(e) => Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                    "Cached error: {}",
                    e
                ))),
            };
        }

        let receiver = self.receiver.lock();
        match receiver.try_recv() {
            Ok(result) => {
                *self.is_complete.lock() = true;
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
        let cache = self.result_cache.lock();
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
                let receiver = self.receiver.lock();
                receiver.recv()
            })
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        *self.is_complete.lock() = true;

        let mut cache = self.result_cache.lock();
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

// =============================================================================
// TASK DEPENDENCY SYSTEM
// =============================================================================

/// Wait for dependencies to complete
fn wait_for_dependencies(dependencies: &[String]) -> PyResult<Vec<Py<PyAny>>> {
    let mut results = Vec::new();

    for dep_id in dependencies {
        // Wait for dependency result to be available
        let mut attempts = 0;
        let max_attempts = 6000; // 10 minutes max wait

        loop {
            // CRITICAL FIX: Check shutdown flag
            if is_shutdown_requested() {
                warn!("Dependency wait cancelled: shutdown in progress");
                return Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                    "Dependency wait cancelled: shutdown in progress"
                ));
            }

            // CRITICAL FIX: Check for task failures via error storage
            if let Some(error) = TASK_ERRORS.get(dep_id) {
                error!("Dependency {} failed: {}", dep_id, error.value());
                return Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                    format!("Dependency {} failed: {}", dep_id, error.value())
                ));
            }

            if let Some(result) = TASK_RESULTS.get(dep_id) {
                Python::attach(|py| {
                    results.push(result.clone_ref(py));
                });
                break;
            }

            if attempts >= max_attempts {
                error!("Dependency {} timed out after 10 minutes", dep_id);
                return Err(PyErr::new::<pyo3::exceptions::PyTimeoutError, _>(
                    format!("Dependency {} timed out after 10 minutes", dep_id)
                ));
            }

            thread::sleep(Duration::from_millis(100));
            attempts += 1;
        }
    }

    Ok(results)
}

/// Store task result for dependencies
fn store_task_result(task_id: String, result: Py<PyAny>) {
    TASK_RESULTS.insert(task_id, result);
}

/// Clear task result after consumption
fn clear_task_result(task_id: &str) {
    TASK_RESULTS.remove(task_id);
}

/// Store task error for dependency failure propagation
fn store_task_error(task_id: String, error: String) {
    TASK_ERRORS.insert(task_id, error);
}

/// Clear task error
fn clear_task_error(task_id: &str) {
    TASK_ERRORS.remove(task_id);
}

/// Parallel wrapper with dependency support
#[pyclass]
struct ParallelWithDeps {
    func: Py<PyAny>,
}

#[pymethods]
impl ParallelWithDeps {
    #[pyo3(signature = (*args, depends_on=None, timeout=None, **kwargs))]
    fn __call__(
        &self,
        py: Python,
        args: &Bound<'_, PyTuple>,
        depends_on: Option<Vec<Py<AsyncHandle>>>,
        timeout: Option<f64>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<AsyncHandle>> {
        // Extract dependency task IDs
        let dep_ids: Vec<String> = if let Some(deps) = depends_on {
            deps.iter()
                .map(|h| h.borrow(py).get_task_id())
                .collect::<PyResult<Vec<String>>>()?
        } else {
            Vec::new()
        };

        // Check if shutdown is requested
        if is_shutdown_requested() {
            return Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                "Cannot start new tasks: shutdown in progress"
            ));
        }

        wait_for_slot();

        if !check_memory_ok() {
            return Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                "Memory limit reached, cannot start new task"
            ));
        }

        let func = self.func.clone_ref(py);
        let task_id = format!("task_{}", TASK_ID_COUNTER.fetch_add(1, Ordering::Relaxed));
        let task_id_clone = task_id.clone();

        // Register dependencies
        if !dep_ids.is_empty() {
            TASK_DEPENDENCIES.insert(task_id.clone(), dep_ids.clone());
        }

        register_task(task_id.clone());

        let func_name = func
            .bind(py)
            .getattr("__name__")
            .ok()
            .and_then(|n| n.extract::<String>().ok())
            .unwrap_or_else(|| "unknown".to_string());

        let args_py: Py<PyTuple> = args.clone().unbind();
        let kwargs_py: Option<Py<PyDict>> = kwargs.map(|k| k.clone().unbind());

        let (sender, receiver): (Sender<PyResult<Py<PyAny>>>, Receiver<PyResult<Py<PyAny>>>) =
            channel();

        let is_complete = Arc::new(Mutex::new(false));
        let is_complete_clone = is_complete.clone();

        let cancel_token = Arc::new(AtomicBool::new(false));
        let cancel_token_clone = cancel_token.clone();

        let func_name_clone = func_name.clone();
        let start_time = Instant::now();

        if let Some(timeout_secs) = timeout {
            let cancel_token_timeout = cancel_token.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_secs_f64(timeout_secs));
                cancel_token_timeout.store(true, Ordering::Release);
            });
        }

        let handle = py.detach(|| {
            thread::spawn(move || {
                Python::attach(|py| {
                    let exec_start = Instant::now();
                    set_current_task_id(Some(task_id_clone.clone()));

                    // Wait for dependencies first
                    let dep_results = if !dep_ids.is_empty() {
                        match wait_for_dependencies(&dep_ids) {
                            Ok(results) => results,
                            Err(e) => {
                                // CRITICAL FIX: Handle channel send errors
                                if let Err(send_err) = sender.send(Err(e)) {
                                    error!("Failed to send dependency error for task {}: {}", task_id_clone, send_err);
                                    store_task_error(task_id_clone.clone(), format!("Dependency wait failed: {}", send_err));
                                }
                                *is_complete_clone.lock() = true;
                                unregister_task(&task_id_clone);
                                clear_task_progress(&task_id_clone);
                                set_current_task_id(None);
                                return;
                            }
                        }
                    } else {
                        Vec::new()
                    };

                    if is_shutdown_requested() || cancel_token_clone.load(Ordering::Acquire) {
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

                        // CRITICAL FIX: Handle channel send errors
                        if let Err(e) = sender.send(Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                            task_error.__str__()
                        ))) {
                            error!("Failed to send cancellation error for task {}: {}", task_id_clone, e);
                            store_task_error(task_id_clone.clone(), format!("Cancellation failed: {}", e));
                        }
                        *is_complete_clone.lock() = true;
                        unregister_task(&task_id_clone);
                        clear_task_progress(&task_id_clone);
                        set_current_task_id(None);
                        return;
                    }

                    // If we have dependencies, pass their results as first argument
                    let final_result = if !dep_results.is_empty() {
                        // Create new tuple with dependency results + original args
                        let dep_tuple = PyTuple::new(py, dep_results.iter().map(|r| r.bind(py))).unwrap();
                        let mut combined_args = vec![dep_tuple.into_any().unbind()];

                        for arg in args_py.bind(py).iter() {
                            combined_args.push(arg.unbind());
                        }

                        let new_tuple = PyTuple::new(py, combined_args.iter().map(|a| a.bind(py))).unwrap();
                        func.bind(py).call(new_tuple, kwargs_py.as_ref().map(|k| k.bind(py)))
                    } else {
                        func.bind(py).call(args_py.bind(py), kwargs_py.as_ref().map(|k| k.bind(py)))
                    };

                    let exec_time = exec_start.elapsed().as_secs_f64() * 1000.0;

                    let to_send = match final_result {
                        Ok(val) => {
                            record_task_execution(&func_name_clone, exec_time, true);
                            let unbound = val.unbind();
                            store_task_result(task_id_clone.clone(), unbound.clone_ref(py));
                            Ok(unbound)
                        }
                        Err(e) => {
                            record_task_execution(&func_name_clone, exec_time, false);

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

                    let _ = sender.send(to_send);
                    *is_complete_clone.lock() = true;

                    unregister_task(&task_id_clone);
                    clear_task_progress(&task_id_clone);
                    TASK_DEPENDENCIES.remove(&task_id_clone);
                    set_current_task_id(None);
                });
            })
        });

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
}

/// Decorator for parallel execution with dependency support
#[pyfunction]
fn parallel_with_deps(py: Python, func: Py<PyAny>) -> PyResult<Py<ParallelWithDeps>> {
    Py::new(py, ParallelWithDeps { func })
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
                    *is_complete_clone.lock() = true;
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
                    *is_complete_clone.lock() = true;
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
    #[pyo3(signature = (*args, priority=0, timeout=None, **kwargs))]
    fn __call__(
        &self,
        py: Python,
        args: &Bound<'_, PyTuple>,
        priority: i32,
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

        let func = self.func.clone_ref(py);

        // Generate unique task ID
        let task_id = format!("task_{}", TASK_ID_COUNTER.fetch_add(1, Ordering::Relaxed));
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

        let args_py: Py<PyTuple> = args.clone().unbind();
        let kwargs_py: Option<Py<PyDict>> = kwargs.map(|k| k.clone().unbind());

        // Use crossbeam channel for priority queue
        let (sender, receiver) = unbounded();

        let is_complete = Arc::new(Mutex::new(false));
        let cancel_token = Arc::new(AtomicBool::new(false));
        let start_time = Instant::now();

        // Setup timeout if specified
        if let Some(timeout_secs) = timeout {
            let cancel_token_timeout = cancel_token.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_secs_f64(timeout_secs));
                cancel_token_timeout.store(true, Ordering::Release);
            });
        }

        // Create priority task
        let task = PriorityTask {
            priority,
            func,
            args: args_py,
            kwargs: kwargs_py,
            sender,
        };

        // Push to priority queue
        PRIORITY_QUEUE.lock().push(task);

        // Ensure worker is running
        if !PRIORITY_WORKER_RUNNING.load(Ordering::SeqCst) {
            start_priority_worker(py)?;
        }

        // Create full AsyncHandle with all features
        let async_handle = AsyncHandle {
            receiver: Arc::new(Mutex::new({
                // Convert crossbeam receiver to std::sync::mpsc receiver
                // We need to spawn a helper thread to bridge the two channel types
                let (std_sender, std_receiver): (Sender<PyResult<Py<PyAny>>>, Receiver<PyResult<Py<PyAny>>>) = channel();
                let is_complete_clone = is_complete.clone();

                thread::spawn(move || {
                    match receiver.recv() {
                        Ok(result) => {
                            let _ = std_sender.send(result);
                            *is_complete_clone.lock() = true;
                            unregister_task(&task_id_clone);
                        }
                        Err(_) => {
                            let _ = std_sender.send(Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                                "Priority task channel closed unexpectedly"
                            )));
                            *is_complete_clone.lock() = true;
                            unregister_task(&task_id_clone);
                        }
                    }
                });

                std_receiver
            })),
            thread_handle: Arc::new(Mutex::new(None)), // Priority tasks don't have individual thread handles
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
        self.handles.lock().push(async_handle.clone_ref(py));
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
        let handles_guard = self.handles.lock();
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

/// Retry with result caching - combines retry logic with memoization
/// Successful results are cached, failed attempts trigger retries
#[pyfunction]
#[pyo3(signature = (*, max_attempts=3, cache_failures=false))]
fn retry_cached(_py: Python<'_>, max_attempts: usize, cache_failures: bool) -> PyResult<Py<PyAny>> {
    let factory = move |py: Python<'_>, func: Py<PyAny>| -> PyResult<Py<PyAny>> {
        // Use DashMap for thread-safe caching
        let cache: Arc<DashMap<String, PyResult<Py<PyAny>>>> = Arc::new(DashMap::new());

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

            // Check cache
            if let Some(cached) = cache.get(&key) {
                return match cached.value() {
                    Ok(val) => {
                        println!("✓ Cache hit (success): {}", key);
                        Ok(val.clone_ref(py))
                    }
                    Err(e) => {
                        if cache_failures {
                            println!("✗ Cache hit (failure): {}", key);
                            Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                                e.to_string()
                            ))
                        } else {
                            // Don't use cached failures, retry
                            drop(cached);
                            cache.remove(&key);
                            // Continue to retry logic below
                            Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                                "Retrying after cached failure"
                            ))?
                        }
                    }
                };
            }

            // Retry logic with caching
            let mut last_err = None;
            for attempt in 0..max_attempts {
                match func.bind(py).call(args, kwargs) {
                    Ok(res) => {
                        let result = res.unbind();
                        // Cache success
                        cache.insert(key.clone(), Ok(result.clone_ref(py)));
                        println!("✓ Cached successful result: {}", key);
                        return Ok(result);
                    }
                    Err(e) => {
                        println!("✗ Attempt {}/{} failed: {}", attempt + 1, max_attempts, e);
                        last_err = Some(e);

                        if attempt < max_attempts - 1 {
                            thread::sleep(Duration::from_millis(100 * (attempt + 1) as u64));
                        }
                    }
                }
            }

            let final_err = last_err.unwrap();

            // Cache failure if requested
            if cache_failures {
                cache.insert(
                    key.clone(),
                    Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                        final_err.to_string()
                    ))
                );
                println!("✗ Cached failed result: {}", key);
            }

            Err(final_err)
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

// =============================================================================
// UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thread_local_task_id() {
        // Test that thread-local storage works
        assert_eq!(
            CURRENT_TASK_ID.with(|id| id.borrow().clone()),
            None,
            "Initial task_id should be None"
        );

        // Set task_id
        set_current_task_id(Some("test_task_123".to_string()));

        assert_eq!(
            CURRENT_TASK_ID.with(|id| id.borrow().clone()),
            Some("test_task_123".to_string()),
            "Task_id should be set"
        );

        // Clear task_id
        set_current_task_id(None);

        assert_eq!(
            CURRENT_TASK_ID.with(|id| id.borrow().clone()),
            None,
            "Task_id should be cleared"
        );
    }

    #[test]
    fn test_thread_isolation() {
        // Test that thread-local storage is isolated between threads
        use std::thread;
        use std::sync::mpsc::channel;

        let (tx1, rx1) = channel();
        let (tx2, rx2) = channel();

        // Thread 1
        let handle1 = thread::spawn(move || {
            set_current_task_id(Some("thread1_task".to_string()));
            let id = CURRENT_TASK_ID.with(|id| id.borrow().clone());
            tx1.send(id).unwrap();
        });

        // Thread 2
        let handle2 = thread::spawn(move || {
            set_current_task_id(Some("thread2_task".to_string()));
            let id = CURRENT_TASK_ID.with(|id| id.borrow().clone());
            tx2.send(id).unwrap();
        });

        handle1.join().unwrap();
        handle2.join().unwrap();

        let thread1_id = rx1.recv().unwrap();
        let thread2_id = rx2.recv().unwrap();

        assert_eq!(thread1_id, Some("thread1_task".to_string()));
        assert_eq!(thread2_id, Some("thread2_task".to_string()));
        assert_ne!(thread1_id, thread2_id, "Thread IDs should be independent");
    }

    #[test]
    fn test_task_progress_map_insert_and_get() {
        // Test basic progress tracking
        let task_id = "test_progress_task";

        // Insert progress
        TASK_PROGRESS_MAP.insert(task_id.to_string(), 0.5);

        // Retrieve progress
        let progress = TASK_PROGRESS_MAP.get(task_id).map(|p| *p);
        assert_eq!(progress, Some(0.5));

        // Update progress
        TASK_PROGRESS_MAP.insert(task_id.to_string(), 0.75);
        let updated_progress = TASK_PROGRESS_MAP.get(task_id).map(|p| *p);
        assert_eq!(updated_progress, Some(0.75));

        // Clean up
        clear_task_progress(task_id);
        assert_eq!(TASK_PROGRESS_MAP.get(task_id).map(|p| *p), None);
    }

    #[test]
    fn test_clear_task_progress() {
        // Test progress cleanup
        let task_id = "cleanup_test_task";

        TASK_PROGRESS_MAP.insert(task_id.to_string(), 1.0);
        assert!(TASK_PROGRESS_MAP.contains_key(task_id));

        clear_task_progress(task_id);
        assert!(!TASK_PROGRESS_MAP.contains_key(task_id));
    }

    #[test]
    fn test_multiple_tasks_progress() {
        // Test multiple tasks tracking progress independently
        let task1 = "multi_task_1";
        let task2 = "multi_task_2";
        let task3 = "multi_task_3";

        TASK_PROGRESS_MAP.insert(task1.to_string(), 0.3);
        TASK_PROGRESS_MAP.insert(task2.to_string(), 0.6);
        TASK_PROGRESS_MAP.insert(task3.to_string(), 0.9);

        assert_eq!(TASK_PROGRESS_MAP.get(task1).map(|p| *p), Some(0.3));
        assert_eq!(TASK_PROGRESS_MAP.get(task2).map(|p| *p), Some(0.6));
        assert_eq!(TASK_PROGRESS_MAP.get(task3).map(|p| *p), Some(0.9));

        // Clean up
        clear_task_progress(task1);
        clear_task_progress(task2);
        clear_task_progress(task3);
    }

    #[test]
    fn test_task_id_counter_increments() {
        // Test that task ID counter increments
        let start = TASK_ID_COUNTER.load(Ordering::SeqCst);

        let id1 = TASK_ID_COUNTER.fetch_add(1, Ordering::SeqCst);
        let id2 = TASK_ID_COUNTER.fetch_add(1, Ordering::SeqCst);
        let id3 = TASK_ID_COUNTER.fetch_add(1, Ordering::SeqCst);

        assert_eq!(id2, id1 + 1);
        assert_eq!(id3, id2 + 1);
        assert!(id1 >= start);
    }

    #[test]
    fn test_active_tasks_registration() {
        // Test task registration and unregistration
        let initial_count = get_active_task_count();

        register_task("test_task_reg_1".to_string());
        assert_eq!(get_active_task_count(), initial_count + 1);

        register_task("test_task_reg_2".to_string());
        assert_eq!(get_active_task_count(), initial_count + 2);

        unregister_task("test_task_reg_1");
        assert_eq!(get_active_task_count(), initial_count + 1);

        unregister_task("test_task_reg_2");
        assert_eq!(get_active_task_count(), initial_count);
    }

    #[test]
    fn test_shutdown_flag() {
        // Test shutdown flag operations
        reset_shutdown().unwrap();
        assert!(!is_shutdown_requested());

        SHUTDOWN_FLAG.store(true, Ordering::Release);
        assert!(is_shutdown_requested());

        reset_shutdown().unwrap();
        assert!(!is_shutdown_requested());
    }

    #[test]
    fn test_progress_boundaries() {
        // Test progress values at boundaries
        let task_id = "boundary_task";

        // Test 0.0
        TASK_PROGRESS_MAP.insert(task_id.to_string(), 0.0);
        assert_eq!(TASK_PROGRESS_MAP.get(task_id).map(|p| *p), Some(0.0));

        // Test 1.0
        TASK_PROGRESS_MAP.insert(task_id.to_string(), 1.0);
        assert_eq!(TASK_PROGRESS_MAP.get(task_id).map(|p| *p), Some(1.0));

        // Test middle value
        TASK_PROGRESS_MAP.insert(task_id.to_string(), 0.5);
        assert_eq!(TASK_PROGRESS_MAP.get(task_id).map(|p| *p), Some(0.5));

        clear_task_progress(task_id);
    }

    #[test]
    fn test_concurrent_progress_updates() {
        use std::thread;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        // Test concurrent progress updates from multiple threads
        let task_id_base = "concurrent_test";
        let num_threads = 10;
        let updates_per_thread = 100;
        let counter = Arc::new(AtomicU32::new(0));

        let handles: Vec<_> = (0..num_threads)
            .map(|i| {
                let counter = counter.clone();
                thread::spawn(move || {
                    let task_id = format!("{}_{}", task_id_base, i);
                    for j in 0..updates_per_thread {
                        let progress = (j as f64) / (updates_per_thread as f64);
                        TASK_PROGRESS_MAP.insert(task_id.clone(), progress);
                        counter.fetch_add(1, Ordering::SeqCst);
                    }
                    clear_task_progress(&task_id);
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(
            counter.load(Ordering::SeqCst),
            num_threads * updates_per_thread,
            "All progress updates should complete"
        );
    }

    #[test]
    fn test_memory_cleanup() {
        // Test that cleanup actually removes entries
        let task_id = "memory_cleanup_test";

        // Add progress
        TASK_PROGRESS_MAP.insert(task_id.to_string(), 0.5);
        assert!(TASK_PROGRESS_MAP.contains_key(task_id));

        // Clear progress
        clear_task_progress(task_id);

        // Verify it's gone
        assert!(!TASK_PROGRESS_MAP.contains_key(task_id));
        assert_eq!(TASK_PROGRESS_MAP.get(task_id).map(|p| *p), None);
    }

    #[test]
    fn test_task_metrics_recording() {
        // Test that task execution recording works
        reset_metrics().unwrap();

        let func_name = "test_function";
        let duration_ms = 100.0;

        // Record successful execution
        record_task_execution(func_name, duration_ms, true);

        // Verify counters
        assert_eq!(TASK_COUNTER.load(Ordering::SeqCst), 1);
        assert_eq!(COMPLETED_COUNTER.load(Ordering::SeqCst), 1);
        assert_eq!(FAILED_COUNTER.load(Ordering::SeqCst), 0);

        // Record failed execution
        record_task_execution(func_name, duration_ms, false);

        assert_eq!(TASK_COUNTER.load(Ordering::SeqCst), 2);
        assert_eq!(COMPLETED_COUNTER.load(Ordering::SeqCst), 1);
        assert_eq!(FAILED_COUNTER.load(Ordering::SeqCst), 1);

        // Clean up
        reset_metrics().unwrap();
    }

    #[test]
    fn test_max_concurrent_tasks() {
        // Test setting concurrent task limit
        set_max_concurrent_tasks(5).unwrap();
        assert_eq!(*MAX_CONCURRENT_TASKS.lock(), Some(5));

        set_max_concurrent_tasks(10).unwrap();
        assert_eq!(*MAX_CONCURRENT_TASKS.lock(), Some(10));
    }

    #[test]
    fn test_check_memory_ok() {
        // Test memory checking (currently always returns true)
        assert!(check_memory_ok());

        // Set memory limit
        configure_memory_limit(75.0).unwrap();

        // Still returns true (actual memory checking not implemented)
        assert!(check_memory_ok());
    }
}

/// This module is implemented in Rust.
#[pymodule]
fn makeparallel(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Initialize logging (only once)
    let _ = env_logger::try_init();

    // Original decorators
    m.add_function(wrap_pyfunction!(timer, m)?)?;
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
    m.add_function(wrap_pyfunction!(get_current_task_id, m)?)?;

    // Helper functions
    m.add_function(wrap_pyfunction!(gather, m)?)?;
    m.add_class::<ParallelContext>()?;
    m.add_function(wrap_pyfunction!(retry_backoff, m)?)?;
    m.add_function(wrap_pyfunction!(retry_cached, m)?)?;

    // Task dependencies
    m.add_function(wrap_pyfunction!(parallel_with_deps, m)?)?;
    m.add_class::<ParallelWithDeps>()?;

    Ok(())
}
