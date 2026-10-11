#!/bin/sh
# Execute the literal Unix workflow hook against an owned real native archive.
set -eu
repo=$(cd "$(dirname "$0")/../.." && pwd)
root=$(mktemp -d)
root=$(cd "$root" && pwd -P)
trap 'rm -rf "$root"' 0 1 2 3 15
binary=$repo/target/release/tfm
test -f "$binary" || { echo 'hook tests require cargo make release-build' >&2; exit 1; }
command -v perl >/dev/null
output=$(perl -e 'alarm 10; exec @ARGV or die "version execution failed"' "$binary" --version)
case "$output" in
    'tfm '*) CRATE_VERSION=${output#tfm } ;;
    *) echo 'unexpected version output' >&2; exit 1 ;;
esac
case "$(uname -m)" in
    x86_64) arch=x86_64 ;;
    aarch64|arm64) arch=aarch64 ;;
    *) echo 'unsupported native architecture' >&2; exit 1 ;;
esac
case "$(uname -s)" in
    Linux) TARGET=$arch-unknown-linux-gnu ;;
    Darwin) TARGET=$arch-apple-darwin ;;
    *) echo 'unsupported native host' >&2; exit 1 ;;
esac
ARCHIVE_NAME=terminal-file-manager-v$CRATE_VERSION-$TARGET.tar.gz
export ARCHIVE_NAME TARGET CRATE_VERSION
mkdir -p "$root/tests" "$root/.github/workflows" "$root/stage" \
    "$root/target/$TARGET/release"
cp -R "$repo/tests/installers" "$repo/tests/ci" "$root/tests/"
cp "$repo/install.sh" "$root/"
cp "$repo/.github/workflows/release-rehearsal.yml" "$root/.github/workflows/"
cp "$binary" "$root/target/$TARGET/release/tfm"
install -m 755 "$binary" "$root/stage/tfm"
cp "$repo/LICENSE-MIT" "$repo/LICENSE-APACHE" "$repo/README.md" "$root/stage/"
sed -n '/name: Install actual Unix release archive offline/,/name: Install actual Windows release archive (PowerShell 7)/p' \
    "$repo/.github/workflows/release-rehearsal.yml" |
    sed '1,/run: |/d; $d; s/^          //' >"$root/check.sh"
test -s "$root/check.sh"
cd "$root"
tar -czf "$ARCHIVE_NAME" -C stage tfm LICENSE-MIT LICENSE-APACHE README.md
case "$(uname -s)" in
    Linux) sha256sum "$ARCHIVE_NAME" >"$ARCHIVE_NAME.sha256" ;;
    Darwin) shasum -a 256 "$ARCHIVE_NAME" >"$ARCHIVE_NAME.sha256" ;;
esac
sh -eu check.sh >output 2>&1 || { cat output; exit 1; }
for result in 'fresh passed' 'reinstall passed' 'corruption rejected; old bytes preserved'; do
    grep -Fq "Real archive $TARGET: $result" output || {
        cat output; echo "missing actual installer proof: $result" >&2; exit 1
    }
done
printf 'corrupt\n' >>"$ARCHIVE_NAME"
if sh -eu check.sh >output 2>&1; then
    echo 'workflow hook accepted a corrupt archive' >&2; exit 1
fi
rm "$ARCHIVE_NAME"
if sh -eu check.sh >output 2>&1; then
    echo 'workflow hook accepted a missing archive' >&2; exit 1
fi
echo 'Real Unix workflow hook: native fresh/reinstall/preservation pass; corrupt and missing archives fail'
