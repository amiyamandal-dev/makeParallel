# Version Management Guide - makeParallel

## How to Bump Version Numbers

### Quick Steps

When releasing a new version, you need to update **TWO files**:

1. **`Cargo.toml`** - Rust package version
2. **`pyproject.toml`** - Python package version

Both must have the **same version number** or builds will fail.

---

## Step-by-Step Process

### 1. Decide on Version Number

Follow [Semantic Versioning](https://semver.org/):

- **MAJOR** (X.0.0) - Breaking changes, incompatible API changes
- **MINOR** (0.X.0) - New features, backwards-compatible
- **PATCH** (0.0.X) - Bug fixes, backwards-compatible

**Examples:**
- `0.1.0` → `0.1.1` - Bug fixes only
- `0.1.1` → `0.2.0` - New features (callbacks, dependencies)
- `0.2.0` → `1.0.0` - Stable release with possible breaking changes

### 2. Update Cargo.toml

**File**: `/Cargo.toml`

```toml
[package]
name = "makeparallel"
version = "0.2.0"  # ← Change this
edition = "2021"
```

**Example change:**
```bash
# From
version = "0.1.1"

# To
version = "0.2.0"
```

### 3. Update pyproject.toml

**File**: `/pyproject.toml`

```toml
[project]
name = "makeparallel"
version = "0.2.0"  # ← Change this
description = "..."
```

**Example change:**
```bash
# From
version = "0.1.1"

# To
version = "0.2.0"
```

### 4. Update CHANGELOG.md

Add a new section at the top:

```markdown
## [0.2.0] - 2025-11-30

### Added
- New feature X
- New feature Y

### Fixed
- Bug fix A
- Bug fix B

### Changed
- API change C
```

### 5. Build and Test

```bash
# Activate virtual environment
source .venv/bin/activate  # or .venv\Scripts\activate on Windows

# Build with new version
maturin develop --release

# Verify version
python -c "import makeparallel; print(makeparallel.__version__)"

# Run all tests
python tests/test_all.py
python test_simple_callbacks.py
python test_progress_fix.py
```

### 6. Commit and Tag

```bash
# Commit version bump
git add Cargo.toml pyproject.toml CHANGELOG.md
git commit -m "Bump version to 0.2.0"

# Create git tag
git tag -a v0.2.0 -m "Release version 0.2.0"

# Push with tags
git push origin main
git push origin v0.2.0
```

### 7. Build Distribution Wheels

```bash
# Build wheels for distribution
maturin build --release

# Wheels will be in target/wheels/
ls target/wheels/
# makeparallel-0.2.0-cp38-cp38-macosx_11_0_arm64.whl
# makeparallel-0.2.0-cp39-cp39-macosx_11_0_arm64.whl
# etc.
```

### 8. Publish to PyPI (Optional)

```bash
# First time only: Install twine
pip install twine

# Upload to TestPyPI (test first!)
twine upload --repository testpypi target/wheels/*

# Verify installation from TestPyPI
pip install --index-url https://test.pypi.org/simple/ makeparallel

# Upload to PyPI (production)
maturin publish

# Or use twine:
twine upload target/wheels/*
```

---

## Version History

### Current Versions

| Version | Date | Changes |
|---------|------|---------|
| 0.2.0 | 2025-11-30 | Callbacks, Dependencies, 24 bug fixes |
| 0.1.1 | 2025-11-29 | Metadata sync, docs update |
| 0.1.0 | 2025-11-28 | Initial release |

---

## Common Issues

### Issue 1: Version Mismatch Error

**Error:**
```
Error: Version mismatch between Cargo.toml (0.2.0) and pyproject.toml (0.1.1)
```

**Solution:**
Make sure both files have the exact same version number.

### Issue 2: Build Fails After Version Bump

**Error:**
```
error: failed to parse manifest at `Cargo.toml`
```

**Solution:**
Check for typos in version number. Must be format: `X.Y.Z`

### Issue 3: Git Tag Already Exists

**Error:**
```
fatal: tag 'v0.2.0' already exists
```

**Solution:**
```bash
# Delete local tag
git tag -d v0.2.0

# Delete remote tag (if pushed)
git push origin :refs/tags/v0.2.0

# Create new tag
git tag -a v0.2.0 -m "Release version 0.2.0"
```

### Issue 4: PyPI Upload Fails

**Error:**
```
HTTPError: 400 Bad Request - File already exists
```

**Solution:**
You cannot re-upload the same version to PyPI. You must bump the version number.

---

## Automation Script

Create `bump_version.sh`:

```bash
#!/bin/bash

# Usage: ./bump_version.sh 0.2.0

NEW_VERSION=$1

if [ -z "$NEW_VERSION" ]; then
    echo "Usage: ./bump_version.sh <version>"
    echo "Example: ./bump_version.sh 0.2.0"
    exit 1
fi

echo "Bumping version to $NEW_VERSION..."

# Update Cargo.toml
sed -i.bak "s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml

# Update pyproject.toml
sed -i.bak "s/^version = \".*\"/version = \"$NEW_VERSION\"/" pyproject.toml

# Remove backup files
rm Cargo.toml.bak pyproject.toml.bak

echo "✅ Version updated to $NEW_VERSION"
echo ""
echo "Next steps:"
echo "1. Update CHANGELOG.md"
echo "2. Run: maturin develop --release"
echo "3. Run tests"
echo "4. Commit: git commit -am 'Bump version to $NEW_VERSION'"
echo "5. Tag: git tag -a v$NEW_VERSION -m 'Release version $NEW_VERSION'"
echo "6. Push: git push origin main --tags"
```

Make it executable:
```bash
chmod +x bump_version.sh
```

Usage:
```bash
./bump_version.sh 0.2.0
```

---

## Checklist for New Release

Use this checklist when releasing a new version:

- [ ] Decide on version number (MAJOR.MINOR.PATCH)
- [ ] Update `Cargo.toml` version
- [ ] Update `pyproject.toml` version
- [ ] Update `CHANGELOG.md` with changes
- [ ] Update `README.md` if needed
- [ ] Build: `maturin develop --release`
- [ ] Run all tests: `python tests/test_all.py`
- [ ] Run callback tests: `python test_simple_callbacks.py`
- [ ] Run progress tests: `python test_progress_fix.py`
- [ ] Commit: `git commit -am "Bump version to X.Y.Z"`
- [ ] Tag: `git tag -a vX.Y.Z -m "Release version X.Y.Z"`
- [ ] Push: `git push origin main --tags`
- [ ] Build wheels: `maturin build --release`
- [ ] Test PyPI upload: `twine upload --repository testpypi target/wheels/*`
- [ ] Publish to PyPI: `maturin publish`
- [ ] Create GitHub release with changelog
- [ ] Announce on social media/forums

---

## GitHub Releases

### Creating a Release on GitHub

1. Go to: https://github.com/amiyamandal-dev/makeParallel/releases
2. Click "Draft a new release"
3. Choose tag: `v0.2.0`
4. Release title: `v0.2.0 - Callbacks, Dependencies, and Critical Bug Fixes`
5. Description: Copy from CHANGELOG.md
6. Attach wheels from `target/wheels/`
7. Check "Set as the latest release"
8. Click "Publish release"

### Release Notes Template

```markdown
# makeParallel v0.2.0

## 🎉 Major Features

- **Callback System** - Event-driven task monitoring
- **Task Dependencies** - Build complex pipelines
- **Auto Progress Tracking** - Simplified API

## 🐛 Bug Fixes

- Fixed 24 critical bugs including deadlocks and memory leaks
- ~10% performance improvement
- All 45 tests passing

## 📥 Installation

```bash
pip install makeparallel==0.2.0
```

## 📝 Full Changelog

See [CHANGELOG.md](CHANGELOG.md) for complete details.
```

---

## PyPI Publishing

### First Time Setup

```bash
# Create ~/.pypirc
cat > ~/.pypirc << EOF
[distutils]
index-servers =
    pypi
    testpypi

[pypi]
username = __token__
password = pypi-YOUR-TOKEN-HERE

[testpypi]
repository = https://test.pypi.org/legacy/
username = __token__
password = pypi-YOUR-TESTPYPI-TOKEN-HERE
EOF

chmod 600 ~/.pypirc
```

### Get API Token

1. Go to https://pypi.org/manage/account/token/
2. Create new token
3. Copy token to `~/.pypirc`

### Publishing Process

```bash
# Build
maturin build --release

# Test on TestPyPI first
maturin publish --repository testpypi

# Install from TestPyPI to verify
pip install --index-url https://test.pypi.org/simple/ makeparallel==0.2.0

# If all good, publish to PyPI
maturin publish
```

---

## Version Naming Convention

| Version | Meaning | Example |
|---------|---------|---------|
| 0.x.y | Pre-1.0, still in development | 0.2.0 |
| 1.0.0 | First stable release | 1.0.0 |
| 1.1.0 | New features, backwards compatible | 1.1.0 |
| 1.1.1 | Bug fixes only | 1.1.1 |
| 2.0.0 | Breaking changes | 2.0.0 |

### When to Bump Major Version (X.0.0)

- Removing features or APIs
- Changing function signatures in incompatible ways
- Changing default behaviors that could break existing code
- First stable release (0.x.x → 1.0.0)

### When to Bump Minor Version (0.X.0)

- Adding new features
- Adding new decorators or functions
- Deprecating features (with warnings)
- Performance improvements
- New dependencies

### When to Bump Patch Version (0.0.X)

- Bug fixes only
- Documentation updates
- Internal refactoring
- Security patches

---

## Summary

**Key Points:**
1. Always update both `Cargo.toml` and `pyproject.toml`
2. Follow semantic versioning
3. Update CHANGELOG.md
4. Test thoroughly before publishing
5. Tag releases in git
6. Publish to PyPI for users to install

**Current Version: 0.2.0**

Last updated: 2025-11-30
