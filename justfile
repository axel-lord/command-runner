default:
	just --list

build-exe:
	cargo build --target x86_64-pc-windows-gnu --release

exe: build-exe
	cp ./target/x86_64-pc-windows-gnu/release/command-runner.exe ./

# Run all tests with all features.
test-all:
	cargo test --all --all-features

# Run tests with all features.
test  *EXTRA:
	cargo test --all-features {{EXTRA}}

# Run tests using miri
test-miri *EXTRA:
	cargo miri test {{EXTRA}}

# Format crates.
fmt:
	cargo fmt --all

# Check all features and targets
check:
	cargo clippy --all --all-features --all-targets --workspace
