#!/bin/bash

set -euo pipefail

RELEASES_MD="RELEASES.md"
CHECKSUMS_FILE="artifacts/checksums.txt"
CONTRACTS_DIR="contracts"
WORKTREE_DIR=".release-check-worktree"

echo "🔍 Validating topmost entries from RELEASES.md..."

TMP_RELEASES=$(mktemp)
SEEN_CONTRACTS=""

# Extract only the first (topmost) entry per contract
grep '^|' "$RELEASES_MD" | tail -n +3 | while IFS='|' read -r _ version contract commit artifact checksum _; do
    version=$(echo "$version" | xargs | sed 's/^v//')
    contract=$(echo "$contract" | xargs)
    commit=$(echo "$commit" | xargs)
    artifact=$(basename "$(echo "$artifact" | xargs)")
    checksum=$(echo "$checksum" | xargs)
    
    echo "$SEEN_CONTRACTS" | grep -q "^$contract$" && continue
    SEEN_CONTRACTS="$SEEN_CONTRACTS\n$contract"
    
    echo "$contract|$version|$commit|$artifact|$checksum"
done > "$TMP_RELEASES"

errors=0
rm -rf "$WORKTREE_DIR"

while IFS='|' read -r contract version commit artifact checksum_expected; do
    echo ""
    echo "🔧 Checking $contract at commit $commit"
    
    # Add clean worktree
    git worktree add -f "$WORKTREE_DIR" "$commit" > /dev/null 2>&1
    
    CARGO_TOML="$WORKTREE_DIR/$CONTRACTS_DIR/$contract/Cargo.toml"
    CHECKSUMS_PATH="$WORKTREE_DIR/$CHECKSUMS_FILE"
    
    # --- Version check ---
    if [ ! -f "$CARGO_TOML" ]; then
        echo "❌ Missing Cargo.toml at $CARGO_TOML"
        errors=$((errors + 1))
        git worktree remove "$WORKTREE_DIR" --force
        continue
    fi
    
    version_cargo=$(grep '^version' "$CARGO_TOML" | head -n1 | cut -d '"' -f2)
    if [ "$version_cargo" != "$version" ]; then
        echo "❌ Version mismatch: Cargo.toml=$version_cargo vs RELEASES.md=$version"
        errors=$((errors + 1))
    fi
    
    # --- Checksum check ---
    if [ ! -f "$CHECKSUMS_PATH" ]; then
        echo "❌ Missing checksums.txt at $CHECKSUMS_PATH"
        errors=$((errors + 1))
        git worktree remove "$WORKTREE_DIR" --force
        continue
    fi
    
    checksum_recorded=$(grep " $artifact\$" "$CHECKSUMS_PATH" | awk '{print $1}')
    if [ "$checksum_recorded" != "$checksum_expected" ]; then
        echo "❌ Checksum mismatch: checksums.txt=$checksum_recorded vs RELEASES.md=$checksum_expected"
        errors=$((errors + 1))
    fi
    
    git worktree remove "$WORKTREE_DIR" --force > /dev/null 2>&1
done < "$TMP_RELEASES"

rm "$TMP_RELEASES"

echo ""
if [ "$errors" -eq 0 ]; then
    echo "✅ All checks passed!"
else
    echo "❌ $errors issues found."
    exit 1
fi
