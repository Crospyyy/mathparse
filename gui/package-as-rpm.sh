cargo build --release
strip -s target/release/quickmafs
cargo generate-rpm