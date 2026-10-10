#!/usr/bin/env bash
# Run the workflow's actual inventory comparison against isolated local assets.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT
export RUNNER_TEMP="$fixture"
export TAG_NAME=v0.1.0
sed -n '/name: Verify exact rehearsal asset inventory/,$p' \
  "$root/.github/workflows/release-rehearsal.yml" |
  sed '1,/run: |/d; s/^          //' > "$fixture/check.sh"
test -s "$fixture/check.sh"
mkdir "$fixture/dist"
cd "$fixture"
for entry in \
  x86_64-unknown-linux-gnu:tar.gz \
  x86_64-unknown-linux-musl:tar.gz \
  aarch64-unknown-linux-gnu:tar.gz \
  aarch64-unknown-linux-musl:tar.gz \
  x86_64-apple-darwin:tar.gz \
  aarch64-apple-darwin:tar.gz \
  x86_64-pc-windows-msvc:zip \
  aarch64-pc-windows-msvc:zip
do
  archive="terminal-file-manager-$TAG_NAME-${entry%%:*}.${entry#*:}"
  printf 'fixture\n' > "dist/$archive"
  printf 'fixture checksum\n' > "dist/$archive.sha256"
done
bash -eu "$fixture/check.sh"

reject() {
  if bash -eu "$fixture/check.sh" > "$fixture/output" 2>&1; then
    echo "error: inventory accepted $1" >&2
    exit 1
  fi
}

touch dist/unexpected-empty
reject 'an unexpected empty asset'
rm dist/unexpected-empty
expected=dist/terminal-file-manager-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
cp "$expected" "$fixture/saved"
legacy=dist/tfm-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
mv "$expected" "$legacy"
mv "$expected.sha256" "$legacy.sha256"
reject 'a legacy executable-based archive basename'
mv "$legacy" "$expected"
mv "$legacy.sha256" "$expected.sha256"
: > "$expected"
reject 'an expected empty asset'
rm "$expected"
reject 'a missing asset'
cp "$fixture/saved" "$expected"
printf 'extra\n' > dist/unexpected
reject 'an unexpected nonempty asset'
rm dist/unexpected
mkdir dist/UNEXPECTED
touch dist/UNEXPECTED/empty-asset
reject 'an unexpected nested empty asset'
printf 'extra\n' > dist/UNEXPECTED/empty-asset
reject 'an unexpected nested nonempty asset'
rm dist/UNEXPECTED/empty-asset
rmdir dist/UNEXPECTED
bash -eu "$fixture/check.sh"
echo 'Release inventory: valid complete inventories pass; empty, missing, extra and nested assets fail.'
