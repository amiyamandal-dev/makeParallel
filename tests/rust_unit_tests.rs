// Standalone Rust unit tests that don't require Python runtime
// These tests verify the core Rust functionality without PyO3

use std::sync::atomic::{AtomicU32, AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use dashmap::DashMap;

#[test]
fn test_dashmap_concurrent_access() {
    // Test that DashMap works correctly with concurrent access
    let map: Arc<DashMap<String, f64>> = Arc::new(DashMap::new());
    let num_threads = 10;
    let ops_per_thread = 100;

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let map_clone = map.clone();
            thread::spawn(move || {
                let key = format!("task_{}", i);
                for j in 0..ops_per_thread {
                    let progress = (j as f64) / (ops_per_thread as f64);
                    map_clone.insert(key.clone(), progress);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify all tasks have their final progress
    for i in 0..num_threads {
        let key = format!("task_{}", i);
        assert!(map.contains_key(&key));
        let progress = map.get(&key).map(|p| *p);
        assert!(progress.is_some());
        assert!(progress.unwrap() >= 0.99); // Should be close to 1.0
    }
}

#[test]
fn test_atomic_counter() {
    let counter = Arc::new(AtomicU32::new(0));
    let num_threads = 5;
    let increments = 1000;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let counter_clone = counter.clone();
            thread::spawn(move || {
                for _ in 0..increments {
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(counter.load(Ordering::SeqCst), num_threads * increments);
}

#[test]
fn test_thread_local_isolation() {
    use std::cell::RefCell;

    thread_local! {
        static TEST_VAR: RefCell<Option<String>> = RefCell::new(None);
    }

    let (tx1, rx1) = std::sync::mpsc::channel();
    let (tx2, rx2) = std::sync::mpsc::channel();

    let handle1 = thread::spawn(move || {
        TEST_VAR.with(|var| {
            *var.borrow_mut() = Some("thread1".to_string());
        });
        thread::sleep(Duration::from_millis(10));
        let value = TEST_VAR.with(|var| var.borrow().clone());
        tx1.send(value).unwrap();
    });

    let handle2 = thread::spawn(move || {
        TEST_VAR.with(|var| {
            *var.borrow_mut() = Some("thread2".to_string());
        });
        thread::sleep(Duration::from_millis(10));
        let value = TEST_VAR.with(|var| var.borrow().clone());
        tx2.send(value).unwrap();
    });

    handle1.join().unwrap();
    handle2.join().unwrap();

    let val1 = rx1.recv().unwrap();
    let val2 = rx2.recv().unwrap();

    assert_eq!(val1, Some("thread1".to_string()));
    assert_eq!(val2, Some("thread2".to_string()));
}

#[test]
fn test_dashmap_remove() {
    let map: DashMap<String, f64> = DashMap::new();

    map.insert("task1".to_string(), 0.5);
    assert!(map.contains_key("task1"));

    map.remove("task1");
    assert!(!map.contains_key("task1"));
}

#[test]
fn test_atomic_bool_flag() {
    let flag = Arc::new(AtomicBool::new(false));

    assert!(!flag.load(Ordering::SeqCst));

    flag.store(true, Ordering::SeqCst);
    assert!(flag.load(Ordering::SeqCst));

    flag.store(false, Ordering::SeqCst);
    assert!(!flag.load(Ordering::SeqCst));
}

#[test]
fn test_progress_value_boundaries() {
    let map: DashMap<String, f64> = DashMap::new();

    // Test 0.0
    map.insert("task".to_string(), 0.0);
    assert_eq!(map.get("task").map(|p| *p), Some(0.0));

    // Test 1.0
    map.insert("task".to_string(), 1.0);
    assert_eq!(map.get("task").map(|p| *p), Some(1.0));

    // Test 0.5
    map.insert("task".to_string(), 0.5);
    assert_eq!(map.get("task").map(|p| *p), Some(0.5));
}

#[test]
fn test_concurrent_dashmap_updates() {
    let map: Arc<DashMap<String, u32>> = Arc::new(DashMap::new());
    let num_threads = 10;
    let task_id = "shared_task";

    map.insert(task_id.to_string(), 0);

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let map_clone = map.clone();
            thread::spawn(move || {
                for _ in 0..100 {
                    map_clone.alter(task_id, |_, v| v + 1);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let final_value = map.get(task_id).map(|v| *v).unwrap();
    assert_eq!(final_value, num_threads * 100);
}
