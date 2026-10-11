#!/bin/sh
# Execute D9's actual native-hash workflow branch without requiring coreutils.
set -eu
root=$(mktemp -d)
trap 'rm -rf "$root"' 0 1 2 3 15
repo=$(cd "$(dirname "$0")/../.." && pwd)
sed -n "/if \[ \"\$ASSET_KIND\" = macos \]; then/,/          else/p" \
    "$repo/.github/workflows/release-rehearsal.yml" |
    sed '1d; $d; s/^            //' >"$root/hash.sh"
test -s "$root/hash.sh"
ARCHIVE_NAME=terminal-file-manager-v0.1.0-x86_64-apple-darwin.tar.gz
export ARCHIVE_NAME
cd "$root"
printf 'archive fixture\n' >"$ARCHIVE_NAME"
sh -eu hash.sh
shasum -a 256 -c "$ARCHIVE_NAME.sha256"
test "$(wc -l <"$ARCHIVE_NAME.sha256")" -eq 1
grep -Eq "^[0-9a-f]{64}  $ARCHIVE_NAME$" "$ARCHIVE_NAME.sha256"
printf 'corrupt\n' >>"$ARCHIVE_NAME"
if shasum -a 256 -c "$ARCHIVE_NAME.sha256" >output 2>&1; then
    echo 'hash check accepted corruption' >&2; exit 1
fi
echo 'D9 native shasum: exact record accepted; corrupted data rejected'
