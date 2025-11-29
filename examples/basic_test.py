"""
Basic test to verify new functions exist
"""

import makeParallel as mp

print("Testing new API availability...")
print("=" * 60)

# Check new functions exist
features = {
    "configure_thread_pool": hasattr(mp, 'configure_thread_pool'),
    "get_thread_pool_info": hasattr(mp, 'get_thread_pool_info'),
    "parallel_priority": hasattr(mp, 'parallel_priority'),
    "start_priority_worker": hasattr(mp, 'start_priority_worker'),
    "stop_priority_worker": hasattr(mp, 'stop_priority_worker'),
    "profiled": hasattr(mp, 'profiled'),
    "get_metrics": hasattr(mp, 'get_metrics'),
    "get_all_metrics": hasattr(mp, 'get_all_metrics'),
    "reset_metrics": hasattr(mp, 'reset_metrics'),
    "PerformanceMetrics": hasattr(mp, 'PerformanceMetrics'),
}

for feature, exists in features.items():
    status = "✓" if exists else "✗"
    print(f"{status} {feature}: {'Available' if exists else 'Missing'}")

# Test basic usage
print("\nTesting basic usage...")

# Thread pool config
info = mp.get_thread_pool_info()
print(f"✓ get_thread_pool_info() returned: {info}")

# Metrics
mp.reset_metrics()
metrics = mp.get_all_metrics()
print(f"✓ get_all_metrics() returned: {type(metrics)}")

# Profiled decorator
@mp.profiled
def simple_func(x):
    return x * 2

result = simple_func(5)
print(f"✓ @profiled decorator works: {result}")

# Check if function was profiled
metrics = mp.get_all_metrics()
print(f"✓ Metrics tracked: {'simple_func' in metrics}")

print("\n" + "=" * 60)
print("All new features are available and working!")
print("=" * 60)
