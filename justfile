# =============================================
# RDeskMini - justfile
# Run `just` to list all recipes.
# =============================================

set shell := ["bash", "-euo", "pipefail", "-c"]

bin     := "rdeskmini"
version := `grep -m1 '^version' Cargo.toml | cut -d'"' -f2`
exe     := if os() == "windows" { ".exe" } else { "" }
windows_target := "x86_64-pc-windows-msvc"
dist    := "dist"

default:
    @just --list

# === Build & Run ===

# Release build for this machine
build:
    cargo build --release --locked
    @echo "Built target/release/{{bin}}{{exe}}"

# Release build for 64-bit Windows (run on Windows with the MSVC toolchain)
build-windows:
    cargo build --release --locked --target {{windows_target}}
    @echo "Built target/{{windows_target}}/release/{{bin}}.exe"

# Debug build
build-debug:
    cargo build --locked

# Run RDeskMini (it both shares this computer and can control another one)
run:
    cargo run --locked

# Build the relay server (run it on a machine with a public address)
server-build:
    cd server && cargo build --release

# Run the relay server locally for testing
server-run:
    cd server && cargo run --release

# Package the release binary as dist/rdeskmini-<version>-<os>-<arch>
package: build
    mkdir -p {{dist}}
    cp target/release/{{bin}}{{exe}} {{dist}}/{{bin}}-{{version}}-{{os()}}-{{arch()}}{{exe}}
    @echo "Packaged {{dist}}/{{bin}}-{{version}}-{{os()}}-{{arch()}}{{exe}}"

# Package the 64-bit Windows release binary
package-windows: build-windows
    mkdir -p {{dist}}
    cp target/{{windows_target}}/release/{{bin}}.exe {{dist}}/{{bin}}-{{version}}-windows-x86_64.exe
    @echo "Packaged {{dist}}/{{bin}}-{{version}}-windows-x86_64.exe"

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
