#!/usr/bin/env bash
# Builds every Lambda binary for arm64 and stages each as target/lambda/<function>.zip.
#
# cargo-lambda writes target/lambda/<bin>/bootstrap.zip. osls names each uploaded artifact by
# its file name alone, so eight files all called bootstrap.zip collide on one S3 key: every
# function would get whichever zip uploaded last. Unique file names avoid that.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo lambda build --release --arm64 --output-format zip -p jobs

for dir in target/lambda/*/; do
  name="$(basename "$dir")"
  cp "${dir}bootstrap.zip" "target/lambda/${name}.zip"
done
ls -l target/lambda/*.zip
