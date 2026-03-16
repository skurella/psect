#!/bin/bash
FILE=$(jq -r '.tool_input.file_path // empty')
[[ "$FILE" == *.rs ]] && cargo fmt --manifest-path "$(git rev-parse --show-toplevel)/Cargo.toml"
exit 0
