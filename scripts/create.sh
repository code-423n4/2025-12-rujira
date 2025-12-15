#!/bin/bash

# Set the replacement string from the first argument
replacement=$1

# Replace hyphens with underscores in the replacement string for Rust compatibility
rust_friendly_replacement=${replacement//-/_}

# Use $rust_friendly_replacement where Rust module or filename is needed
cp -r contracts/rujira-template contracts/rujira-$replacement
cp packages/rujira-rs/src/interfaces/template.rs packages/rujira-rs/src/interfaces/$rust_friendly_replacement.rs
cd contracts/rujira-$replacement
grep -rl 'rujira-template' . | xargs sed -i "s/rujira-template/rujira-$replacement/g"
grep -rl 'rujira_rs::template' . | xargs sed -i "s/rujira_rs::template/rujira_rs::$rust_friendly_replacement/g"
grep -rl 'template::' ./src/bin/schema.rs | xargs sed -i "s/template::/$rust_friendly_replacement::/g"
sed -i "1i pub mod $rust_friendly_replacement;" ../../packages/rujira-rs/src/interfaces/mod.rs
cargo run schema
cd -
cargo fmt
