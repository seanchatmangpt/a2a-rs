# Installation Guide for a2a-construct (Python Bindings)

This guide covers installing the `a2a-construct` Python package from source.

## Prerequisites

### System Requirements

- **Rust**: 1.85 or later
- **Python**: 3.8 or later
- **Maturin**: Python build backend for Rust extensions

### Install Rust

If you don't have Rust installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### Install Maturin

```bash
pip install maturin
```

## Installation Methods

### 1. Development Installation (Recommended for Development)

This installs the package in editable mode with debug symbols:

```bash
cd a2a-py
maturin develop
```

This is the fastest way to develop and test changes. After running this command, you can:

```python
import a2a_construct
state = a2a_construct.OntologyState()
print(state.task_count())
```

### 2. Release Build

Build an optimized wheel:

```bash
cd a2a-py
maturin build --release
```

This creates a wheel in `target/wheels/`. Install it with:

```bash
pip install target/wheels/a2a_construct-*.whl
```

### 3. Direct Installation

Install directly from source:

```bash
cd a2a-py
pip install .
```

## Verification

After installation, verify it works:

```python
>>> import a2a_construct
>>> print(a2a_construct.__version__)
0.1.0
>>> state = a2a_construct.OntologyState()
>>> print(state.is_empty())
True
>>> chain = a2a_construct.ReceiptChain()
>>> chain.add_transition(b"obs", b"act", b"delta")
>>> print(chain.verify_integrity())
True
```

## Running Examples

```bash
cd a2a-py
maturin develop  # Install in dev mode
python examples/basic_usage.py
```

## Running Tests

Install test dependencies:

```bash
pip install pytest
```

Run tests:

```bash
cd a2a-py
maturin develop  # Ensure package is installed
pytest tests/
```

## Troubleshooting

### "maturin: command not found"

Install maturin:

```bash
pip install maturin
```

### "rustc: command not found"

Install Rust toolchain:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Python can't find the module

Make sure you've run `maturin develop` or installed the package:

```bash
cd a2a-py
maturin develop
```

### Import errors about missing features

The package is built with `receipts` and `receipts-signing` features enabled by default. If you see import errors, check that these features are available in the parent `a2a-rs` crate.

## Development Workflow

1. Make changes to `src/lib.rs`
2. Rebuild: `maturin develop`
3. Test: `python -c "import a2a_construct; ..."`
4. Run tests: `pytest tests/`

## Building for Distribution

To build wheels for distribution:

```bash
# Build for current platform
maturin build --release

# Build for multiple Python versions (requires multiple Python installations)
maturin build --release --interpreter python3.8 python3.9 python3.10 python3.11 python3.12
```

Wheels will be in `target/wheels/`.

## Platform-Specific Notes

### Linux

Requires standard build tools:

```bash
sudo apt-get install build-essential python3-dev  # Debian/Ubuntu
sudo yum groupinstall "Development Tools" python3-devel  # RHEL/CentOS
```

### macOS

Requires Xcode Command Line Tools:

```bash
xcode-select --install
```

### Windows

Requires Visual Studio Build Tools or MSVC:

- Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022)
- Select "Desktop development with C++"

## Next Steps

- Read [README.md](README.md) for API documentation
- Try [examples/basic_usage.py](examples/basic_usage.py)
- Explore the test suite in `tests/`
