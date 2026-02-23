.PHONY: help build build-release build-ebpf build-with-ebpf clean check test

# Default target
help:
	@echo "IMDS Interposer Build Targets:"
	@echo "  make build              - Build debug binary"
	@echo "  make build-release      - Build release binary"
	@echo "  make build-ebpf         - Build eBPF program only (Docker/Finch)"
	@echo "  make build-with-ebpf    - Build release binary with embedded eBPF"
	@echo "  make build-all          - Build both main binary and eBPF program separately"
	@echo "  make check              - Run cargo check"
	@echo "  make test               - Run tests"
	@echo "  make clean              - Clean build artifacts"

# Build debug binary
build:
	@echo "Building debug binary..."
	cargo build

# Build release binary
build-release:
	@echo "Building release binary..."
	cargo build --release
	@echo "Binary location: target/release/imdsinterposer"

# Build eBPF program using Docker/Finch
build-ebpf:
	@echo "Building eBPF program using Docker/Finch..."
	@if command -v finch >/dev/null 2>&1; then \
		./scripts/build-ebpf.sh finch; \
	elif command -v docker >/dev/null 2>&1; then \
		./scripts/build-ebpf.sh docker; \
	else \
		echo "Error: Neither finch nor docker found. Please install one of them."; \
		exit 1; \
	fi

# Build with embedded eBPF (requires nightly, uses Docker/Finch on non-Linux)
build-with-ebpf:
	@echo "Building with embedded eBPF support..."
	./scripts/build-linux-native.sh

# Build everything separately
build-all: build-release build-ebpf
	@echo "Build complete!"
	@echo "Main binary: target/release/imdsinterposer"
	@echo "eBPF binary: ebpf/target/bpfel-unknown-none/release/imdsinterposer-ebpf"

# Run cargo check
check:
	cargo check

# Run tests
test:
	cargo test

# Clean build artifacts
clean:
	cargo clean
	cd ebpf && cargo clean
