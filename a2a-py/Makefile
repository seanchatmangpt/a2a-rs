# Makefile for a2a-py development

.PHONY: help dev build test clean install release

help:
	@echo "a2a-py - Python bindings for A2A CONSTRUCT runtime"
	@echo ""
	@echo "Available targets:"
	@echo "  dev      - Build and install in development mode"
	@echo "  build    - Build release wheel"
	@echo "  test     - Run Rust and Python tests"
	@echo "  clean    - Clean build artifacts"
	@echo "  install  - Install from source"
	@echo "  release  - Build release wheel for distribution"

dev:
	maturin develop

build:
	maturin build

release:
	maturin build --release

test: dev
	cargo test
	pytest tests/

clean:
	cargo clean
	rm -rf target/
	rm -rf dist/
	rm -rf build/
	rm -rf *.egg-info
	find . -type d -name __pycache__ -exec rm -rf {} + 2>/dev/null || true
	find . -type f -name "*.pyc" -delete
	find . -type f -name "*.so" -delete

install:
	pip install .

.DEFAULT_GOAL := help
