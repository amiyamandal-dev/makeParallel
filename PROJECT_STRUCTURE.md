# Project Structure

This document describes the organization of the makeParallel project.

## Directory Layout

```
makeParallel/
├── src/                      # Rust source code
│   └── lib.rs               # Main PyO3 implementation
├── tests/                    # Python test suite
│   ├── test_all.py          # Comprehensive test suite (33 tests)
│   ├── test_decorators.py   # Basic decorator tests
│   ├── test_minimal.py      # Minimal smoke tests
│   ├── test_parallel.py     # Parallel decorator tests
│   └── test_parallel_simple.py
├── examples/                 # Example scripts and benchmarks
│   ├── benchmark_optimizations.py
│   ├── comparison_multiprocessing.py
│   ├── prove_true_parallelism.py
│   └── visual_parallelism_proof.py
├── docs/                     # Documentation
│   ├── OPTIMIZATION_GUIDE.md    # Advanced optimization guide
│   ├── PARALLELISM_PROOF.md     # Proof of true parallelism
│   └── README_PARALLEL.md       # Parallel decorator API reference
├── .github/                  # GitHub configuration
├── Cargo.toml               # Rust dependencies
├── pyproject.toml           # Python project metadata
├── README.md                # Main project documentation
├── LICENSE                  # MIT License
└── .gitignore              # Git ignore rules

## Key Files

### Source Code
- **src/lib.rs**: Core Rust implementation using PyO3
  - All decorators (timer, log_calls, CallCounter, retry, memoize)
  - Parallel decorators (parallel, parallel_fast, parallel_pool)
  - Optimized versions (memoize_fast, parallel_map)
  - AsyncHandle for pipe-based communication

### Tests
- **tests/test_all.py**: Main test suite with 33 comprehensive tests
- Run with: `python tests/test_all.py`

### Examples
- **examples/prove_true_parallelism.py**: Demonstrates 3.76x speedup
- **examples/visual_parallelism_proof.py**: Real-time progress visualization
- **examples/benchmark_optimizations.py**: Performance comparison benchmarks

### Documentation
- **README.md**: Main project documentation with quickstart
- **docs/OPTIMIZATION_GUIDE.md**: Deep dive into Rust optimizations
- **docs/PARALLELISM_PROOF.md**: Mathematical proof of GIL-free execution
- **docs/README_PARALLEL.md**: Detailed API reference

## Development Workflow

### Building
```bash
maturin develop --release
```

### Testing
```bash
# Run all tests
python tests/test_all.py

# Run specific tests
python tests/test_minimal.py
```

### Benchmarking
```bash
# Performance comparison
python examples/benchmark_optimizations.py

# True parallelism proof
python examples/prove_true_parallelism.py
```

### Code Quality
```bash
# Format Rust code
cargo fmt

# Lint Rust code
cargo clippy
```

## Dependencies

### Rust (Cargo.toml)
- PyO3 0.27.1 - Python bindings
- Crossbeam 0.8 - Lock-free channels
- Rayon 1.10 - Thread pool
- DashMap 6.1 - Lock-free HashMap
- Once_Cell 1.20 - Lazy initialization

### Python (pyproject.toml)
- maturin - Build tool
- Python 3.8+ required

## Architecture

The project uses PyO3 to create Python extensions in Rust, enabling:
- True parallelism without Python's GIL
- Zero-overhead abstractions
- Memory-safe concurrent execution
- Lock-free data structures

See [docs/OPTIMIZATION_GUIDE.md](docs/OPTIMIZATION_GUIDE.md) for technical details.
