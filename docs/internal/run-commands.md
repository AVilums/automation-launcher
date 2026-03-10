# Build everything
cargo build --workspace

# Run all tests
cargo test --workspace

# Run the launcher
cargo run --bin launcher
cargo run --bin launcher -- --help
cargo run --bin launcher -- cache status
cargo run --bin launcher -- config show

# Run bootstrap (skip network calls for local dev)
cargo run --bin bootstrap -- --skip-update --skip-killswitch --no-launch