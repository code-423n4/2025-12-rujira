for contract_dir in contracts/*/; do
    if [ -f "$contract_dir/Cargo.toml" ]; then
        echo "Generating schema for $(basename $contract_dir)"
        (cd "$contract_dir" && cargo run schema)
    fi
done