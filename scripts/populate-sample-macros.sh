#!/bin/bash

# Copy sample macros beside the development executable, from any working directory.
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
readonly SOURCE_DIR="${SCRIPT_DIR}/../docs/samples/macros"
readonly TARGET_DIR="${SCRIPT_DIR}/../app/src-tauri/target/debug/macros"

shopt -s nullglob
sample_files=("${SOURCE_DIR}"/*.json)
if (( ${#sample_files[@]} == 0 )); then
    printf 'No sample macro JSON files found in %s\n' "${SOURCE_DIR}" >&2
    exit 1
fi

mkdir -p -- "${TARGET_DIR}"
cp -- "${sample_files[@]}" "${TARGET_DIR}/"
