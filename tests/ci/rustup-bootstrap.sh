#!/usr/bin/env bash
# Exercise the production bootstrap with corrupt bytes and real sha256sum.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT
mkdir "$fixture/bin"
export TFM_BOOTSTRAP_SENTINEL="$fixture/executed"
export TFM_BOOTSTRAP_DOWNLOAD="$fixture/download-url"
export TFM_BOOTSTRAP_PAYLOAD="$fixture/payload"
export TFM_BOOTSTRAP_ARGUMENTS="$fixture/arguments"
cat > "$TFM_BOOTSTRAP_PAYLOAD" <<'PAYLOAD'
#!/bin/sh
printf 'UNVERIFIED EXECUTION\n' > "$TFM_BOOTSTRAP_SENTINEL"
printf '%s\n' "$@" > "$TFM_BOOTSTRAP_ARGUMENTS"
PAYLOAD
cat > "$fixture/bin/curl" <<'CURL'
#!/bin/bash
set -eu
output=
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output) output=$2; shift ;;
    https://*) printf '%s\n' "$1" > "$TFM_BOOTSTRAP_DOWNLOAD" ;;
  esac
  shift
done
if [ -n "$output" ]; then
  cp "$TFM_BOOTSTRAP_PAYLOAD" "$output"
else
  cat "$TFM_BOOTSTRAP_PAYLOAD"
fi
CURL
chmod +x "$fixture/bin/curl"

sed -n '/export CARGO_HOME=/,/export PATH=/p' \
  "$root/.github/workflows/release-rehearsal.yml" |
  sed "s|/tmp/tfm-|$fixture/tfm-|g" > "$fixture/bootstrap.sh"
test -s "$fixture/bootstrap.sh"
export RUST_VERSION=1.99.0
export RUSTUP_VERSION=1.29.1
grep -Fxq "  RUSTUP_VERSION: $RUSTUP_VERSION" \
  "$root/.github/workflows/release-rehearsal.yml"
for entry in \
  x86_64-unknown-linux-gnu:dda7234360b7f578ca8b0ddcb80145646fa61a67c1720a5abc7051b35c9fcb71 \
  x86_64-unknown-linux-musl:331228566cc931f32cd684f9bebc5956a5e5d7394c8d5de595f196a658ab98e6 \
  aarch64-unknown-linux-gnu:15f6e4ce9f583b929c996c91562bad6d4454f3281de858b02cdfdef615fac433 \
  aarch64-unknown-linux-musl:1ddf36182ac5d1782dbeefcb9bea7f5f9412d88f4a0d5047b1564a223624e8c6
do
  export TARGET=${entry%%:*}
  export RUSTUP_INIT_SHA256=${entry#*:}
  configured=$(awk -v target="$TARGET" '
    $0 ~ "target: " target "$" { selected = 1; next }
    selected && /rustup_sha256:/ { print $2; exit }
  ' "$root/.github/workflows/release-rehearsal.yml")
  test "$configured" = "$RUSTUP_INIT_SHA256"
  if PATH="$fixture/bin:$PATH" bash -eu "$fixture/bootstrap.sh" > "$fixture/output" 2>&1; then
    echo "error: corrupt bootstrap succeeded for $TARGET" >&2
    exit 1
  fi
  test ! -e "$TFM_BOOTSTRAP_SENTINEL"
  grep -Fq 'FAILED' "$fixture/output"
  test "$(cat "$TFM_BOOTSTRAP_DOWNLOAD")" = \
    "https://static.rust-lang.org/rustup/archive/$RUSTUP_VERSION/$TARGET/rustup-init"
done
echo 'Rustup corruption regression: all four Linux targets blocked before execution.'

# Positive control: the same real checksum boundary accepts known fixture bytes.
RUSTUP_INIT_SHA256=$(sha256sum "$TFM_BOOTSTRAP_PAYLOAD")
export RUSTUP_INIT_SHA256=${RUSTUP_INIT_SHA256%% *}
PATH="$fixture/bin:$PATH" bash -eu "$fixture/bootstrap.sh" > "$fixture/output" 2>&1
test -s "$TFM_BOOTSTRAP_SENTINEL"
printf '%s\n' -y --no-modify-path --profile minimal --default-toolchain "$RUST_VERSION" \
  > "$fixture/expected-arguments"
cmp "$fixture/expected-arguments" "$TFM_BOOTSTRAP_ARGUMENTS"
echo 'Rustup positive control: verified bytes execute with the reference arguments.'
