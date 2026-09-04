set shell := ["bash", "-euo", "pipefail", "-c"]

patch:
    cargo release patch --no-publish --execute

publish:
    cargo publish

ci:
    cargo fmt --all -- --check
    cargo check --all-targets --all-features --locked
    cargo clippy --all-targets --all-features --locked -- -D warnings
    cargo test --all-features --locked
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --locked
    cargo run --locked -- verify
    cargo package --allow-dirty --locked
