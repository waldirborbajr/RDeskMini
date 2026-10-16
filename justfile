# =============================================
# RDeskMini - justfile
# Run `just` to list all recipes.
# =============================================

set shell := ["bash", "-euo", "pipefail", "-c"]

bin     := "rdeskmini"
version := `grep -m1 '^version' Cargo.toml | cut -d'"' -f2`
exe     := if os() == "windows" { ".exe" } else { "" }
dist    := "dist"

default:
    @just --list

# === Build & Run ===

# Release build for this machine
build:
    cargo build --release --locked
    @echo "Built target/release/{{bin}}{{exe}}"

# Debug build
build-debug:
    cargo build --locked

# Run the host (the computer being controlled)
run-host:
    cargo run --locked -- --mode host

# Run the client (the computer that controls)
run-client:
    cargo run --locked -- --mode client

# Run the host with the Portuguese interface
run-host-pt:
    cargo run --locked -- --mode host --lang pt

# Run the client with the Portuguese interface
run-client-pt:
    cargo run --locked -- --mode client --lang pt

# Package the release binary as dist/rdeskmini-<version>-<os>-<arch>
package: build
    mkdir -p {{dist}}
    cp target/release/{{bin}}{{exe}} {{dist}}/{{bin}}-{{version}}-{{os()}}-{{arch()}}{{exe}}
    @echo "Packaged {{dist}}/{{bin}}-{{version}}-{{os()}}-{{arch()}}{{exe}}"

# === Quality & Linting ===

# Format the code with rustfmt
fmt:
    cargo fmt --all

# Check formatting without changing files
fmt-check:
    cargo fmt --all -- --check

# Clippy on all targets and features; any warning fails the recipe
clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# Apply clippy suggestions automatically, then re-check with -D warnings
fix:
    cargo clippy --fix --allow-dirty --allow-staged --all-targets --all-features -- -D warnings

# Formatting, lints and tests on the host
check: fmt-check clippy test-std

# === Testing ===

# Tests with the standard harness (no extra tool needed)
test-std:
    cargo test --locked

# Tests with cargo-nextest (install: cargo install cargo-nextest --locked)
test:
    cargo nextest run

# Verbose test run with cargo-nextest
test-verbose:
    cargo nextest run --verbose

# nextest suite plus doc tests
test-all: test
    cargo test --doc

# === Audit & Security ===

# Check dependencies for known vulnerabilities (install: cargo install cargo-audit --locked)
audit:
    cargo audit

# Show outdated dependencies (install: cargo install cargo-outdated --locked)
outdated:
    cargo outdated

# Find unused dependencies (needs nightly; install: cargo install cargo-udeps --locked)
udeps:
    cargo +nightly udeps --all-targets

# === Code Analysis ===

# Expand macros (install: cargo install cargo-expand --locked)
expand *ARGS:
    cargo expand {{ARGS}}

# Flame graph of the binary (install: cargo install flamegraph --locked)
flamegraph *ARGS:
    cargo flamegraph {{ARGS}}

# Size per crate in the release binary (install: cargo install cargo-bloat --locked)
size:
    cargo bloat --release --crates

# === Dependencies ===

# Update Cargo.lock within the existing version requirements
update:
    cargo update

# Add a dependency, e.g.: just add serde --features derive
add *ARGS:
    cargo add {{ARGS}}

# Remove a dependency
rm *ARGS:
    cargo remove {{ARGS}}

# === Utils ===

# Re-run a command on file changes (install: cargo install cargo-watch --locked)
watch +ARGS="run":
    cargo watch -x "{{ARGS}}"

# === Full check ===

# Everything that must pass before a release
check-all: clippy test-std fmt-check audit outdated
    @echo "All checks passed!"

# === Cleanup ===

# Remove build output (target/ and dist/)
clean:
    cargo clean
    rm -rf {{dist}}
