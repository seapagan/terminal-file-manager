#!/bin/sh
# Deterministic installer tests for install.sh. All network and tool
# interaction is mocked through PATH shims and local release assets; the
# suite never contacts GitHub.

set -eu

if test "${1:-}" = --real-archive; then
    shift
    exec sh "$(dirname "$0")/unix-real.sh" "$@"
fi

root=$(mktemp -d)
root=$(cd "$root" && pwd -P)
trap 'rm -rf "$root"' 0 1 2 3 15

real_bin=$root/real-bin
test_bin=$root/bin
wget_bin=$root/wget-bin
no_download_bin=$root/no-download-bin
no_sha_bin=$root/no-sha-bin
release_dir=$root/release
mkdir -p "$real_bin" "$test_bin" "$wget_bin" "$no_download_bin" \
    "$no_sha_bin" "$release_dir" "$root/archive" "$root/empty" \
    "$root/installer-tmp"

for command in awk cat chmod cmp cksum cp find grep gzip head ls mkdir pwd rm sed wc; do
    path=$(command -v "$command")
    ln -s "$path" "$real_bin/$command"
done
for directory in "$test_bin" "$wget_bin" "$no_download_bin" "$no_sha_bin"; do
    for path in "$real_bin"/*; do
        ln -s "$path" "$directory/${path##*/}"
    done
done

REAL_INSTALL=$(command -v install)
REAL_MV=$(command -v mv)
if test "$(uname -s)" = Darwin; then
    REAL_SHA256SUM=$root/hash
    real_shasum=$(command -v shasum)
    printf '#!/bin/sh\nexec "%s" -a 256 "$@"\n' "$real_shasum" >"$REAL_SHA256SUM"
    chmod +x "$REAL_SHA256SUM"
else
    REAL_SHA256SUM=$(command -v sha256sum)
fi
REAL_TAR=$(command -v tar)
export REAL_INSTALL REAL_MV REAL_SHA256SUM REAL_TAR

cat >"$test_bin/uname" <<'EOF'
#!/bin/sh
case "$1" in
    -s) printf '%s\n' "${TEST_OS:-Linux}" ;;
    -m) printf '%s\n' "${TEST_ARCH:-x86_64}" ;;
    *) exit 2 ;;
esac
EOF

cat >"$test_bin/getconf" <<'EOF'
#!/bin/sh
printf 'getconf\n' >>"$TEST_EVENT_LOG"
case "${TEST_GETCONF_KIND:-glibc}" in
    glibc) printf 'glibc %s\n' "$TEST_GLIBC_VERSION" ;;
    malformed) printf 'glibc unknown\n' ;;
    unavailable) exit 1 ;;
esac
EOF

cat >"$test_bin/ldd" <<'EOF'
#!/bin/sh
printf 'ldd\n' >>"$TEST_EVENT_LOG"
case "${TEST_LDD_KIND:-glibc}" in
    glibc) printf 'ldd (GNU libc) %s\n' "$TEST_GLIBC_VERSION" ;;
    musl) printf 'musl libc (%s)\n' "${TEST_ARCH:-x86_64}" >&2 ;;
    malformed) printf 'ldd mystery libc\n' ;;
    unavailable) exit 1 ;;
esac
EOF

cat >"$test_bin/mktemp" <<'EOF'
#!/bin/sh
case "$1" in
    -d)
        count=$(cat "$TEST_MKTEMP_COUNTER")
        count=$((count + 1))
        printf '%s\n' "$count" >"$TEST_MKTEMP_COUNTER"
        directory="$TEST_TMP_PARENT/run-$count"
        mkdir "$directory"
        printf '%s\n' "$directory" >"$TEST_MKTEMP_LAST"
        printf '%s\n' "$directory"
        ;;
    *)
        test "$TEST_STAGE_FAIL" != 1 || exit 1
        count=$(cat "$TEST_STAGE_COUNTER")
        count=$((count + 1))
        printf '%s\n' "$count" >"$TEST_STAGE_COUNTER"
        path=${1%XXXXXX}$count
        : >"$path"
        printf '%s\n' "$path" >"$TEST_STAGE_LAST"
        printf '%s\n' "$path"
        ;;
esac
EOF

cat >"$test_bin/curl" <<'EOF'
#!/bin/sh
url=
destination=
while test "$#" -gt 0; do
    case "$1" in
        -o) destination=$2; shift 2 ;;
        -w|--proto|--proto-redir) shift 2 ;;
        -*) shift ;;
        *) url=$1; shift ;;
    esac
done
printf '%s\n' "$url" >>"$TEST_DOWNLOAD_LOG"
printf 'curl\n' >>"$TEST_DOWNLOADER_LOG"
case "$url" in
    */releases/latest) printf '%s\n' "$TEST_LATEST_URL" ;;
    */releases/download/*)
        name=${url##*/}
        cp "$TEST_RELEASE_DIR/$name" "$destination"
        ;;
    *) exit 1 ;;
esac
EOF

cat >"$test_bin/wget" <<'EOF'
#!/bin/sh
url=
destination=
while test "$#" -gt 0; do
    case "$1" in
        -qO) destination=$2; shift 2 ;;
        -*) shift ;;
        *) url=$1; shift ;;
    esac
done
printf '%s\n' "$url" >>"$TEST_DOWNLOAD_LOG"
printf 'wget\n' >>"$TEST_DOWNLOADER_LOG"
case "$url" in
    */releases/latest) printf '  Location: %s\n' "$TEST_LATEST_URL" >&2 ;;
    */releases/download/*)
        name=${url##*/}
        cp "$TEST_RELEASE_DIR/$name" "$destination"
        ;;
    *) exit 1 ;;
esac
EOF

cat >"$test_bin/sha256sum" <<'EOF'
#!/bin/sh
printf 'sha256sum\n' >>"$TEST_EVENT_LOG"
exec "$REAL_SHA256SUM" "$@"
EOF

# shasum shim: lets Darwin-mode runs execute on any Unix by translating
# to the local sha256sum (identical "HASH  NAME" wire format).
cat >"$test_bin/shasum" <<'EOF'
#!/bin/sh
printf 'shasum\n' >>"$TEST_EVENT_LOG"
args=
while test "$#" -gt 0; do
    case "$1" in
        -a) shift 2 ;;
        *) args="$args $1"; shift ;;
    esac
done
exec "$REAL_SHA256SUM" $args
EOF

# Header shim preserves the original synthetic control-flow tests. These
# shell fixtures are not evidence of real ELF/Mach-O identity or execution.
cat >"$test_bin/od" <<'EOF'
#!/bin/sh
case "${TEST_OS:-Linux}" in
    Linux)
        machine=62
        case "${TEST_ARCH:-x86_64}" in arm64|aarch64) machine=183 ;; esac
        printf '127 69 76 70 2 1 1 0 0 0 0 0 0 0 0 0 2 0 %s 0\n' "$machine"
        ;;
    Darwin)
        machine=7
        test "${TEST_ARCH:-x86_64}" != arm64 || machine=12
        case "$machine" in
            7) subtype='3 0 0 128' ;;
            12) subtype='0 0 0 0' ;;
        esac
        printf '207 250 237 254 %s 0 0 1 %s 2 0 0 0 0 0 0 0\n' "$machine" "$subtype"
        ;;
esac
EOF
chmod +x "$test_bin/od"

cat >"$test_bin/tar" <<'EOF'
#!/bin/sh
printf 'tar\n' >>"$TEST_EVENT_LOG"
case "$1:${TEST_TAR_INSPECT_FAIL:-}" in -tzf:names|-tvzf:types) exit 1 ;; esac
exec "$REAL_TAR" "$@"
EOF

cat >"$test_bin/install" <<'EOF'
#!/bin/sh
printf 'install\n' >>"$TEST_EVENT_LOG"
if test "${TEST_CHANGE_FINAL:-0}" = 1; then
    printf 'external replacement\n' >"$TFM_INSTALL_DIR/tfm"
fi
if test "$TEST_INSTALL_SIGNAL" = 1; then
    kill -TERM "$PPID"
    exit 1
fi
test "$TEST_INSTALL_FAIL" != 1 || exit 1
exec "$REAL_INSTALL" "$@"
EOF

cat >"$test_bin/mv" <<'EOF'
#!/bin/sh
printf 'mv\n' >>"$TEST_EVENT_LOG"
test "$TEST_MOVE_FAIL" != 1 || exit 1
exec "$REAL_MV" "$@"
EOF

chmod +x \
    "$test_bin/uname" \
    "$test_bin/getconf" \
    "$test_bin/ldd" \
    "$test_bin/mktemp" \
    "$test_bin/curl" \
    "$test_bin/wget" \
    "$test_bin/sha256sum" \
    "$test_bin/shasum" \
    "$test_bin/tar" \
    "$test_bin/install" \
    "$test_bin/mv"
for name in uname getconf ldd mktemp od tar install mv sha256sum shasum; do
    for directory in "$wget_bin" "$no_download_bin" "$no_sha_bin"; do
        if test "$directory:$name" = "$no_sha_bin:sha256sum"; then
            continue
        fi
        rm -f "$directory/$name"
        cp "$test_bin/$name" "$directory/$name"
    done
done
cp "$test_bin/wget" "$wget_bin/wget"
cp "$test_bin/curl" "$test_bin/wget" "$no_sha_bin/"

cat >"$root/archive/tfm" <<'EOF'
#!/bin/sh
printf 'candidate\n' >>"$TEST_EVENT_LOG"
test "${TEST_CANDIDATE_EXIT:-0}" = 0 || exit "$TEST_CANDIDATE_EXIT"
printf 'tfm %s\n' "${TEST_CANDIDATE_VERSION:-99.88.77}"
EOF
chmod +x "$root/archive/tfm"
printf 'release notices\n' >"$root/archive/THIRD_PARTY_NOTICES.md"
printf 'release readme\n' >"$root/archive/README.md"
printf 'release licence\n' >"$root/archive/LICENSE-MIT"
printf 'release licence\n' >"$root/archive/LICENSE-APACHE"

# Synthetic versions used only by this test harness.
# They deliberately do not correspond to real tfm releases.
TEST_RELEASE_VERSION=99.88.77
TEST_RELEASE_TAG=v$TEST_RELEASE_VERSION
TEST_OLD_VERSION=98.77.66
TEST_OLD_TAG=v$TEST_OLD_VERSION

targets='x86_64-unknown-linux-gnu x86_64-unknown-linux-musl aarch64-unknown-linux-gnu aarch64-unknown-linux-musl x86_64-apple-darwin aarch64-apple-darwin'
for target in $targets; do
    "$REAL_TAR" -czf "$release_dir/terminal-file-manager-$TEST_RELEASE_TAG-$target.tar.gz" \
        -C "$root/archive" tfm README.md THIRD_PARTY_NOTICES.md LICENSE-MIT LICENSE-APACHE
    (cd "$release_dir" && "$REAL_SHA256SUM" \
        "terminal-file-manager-$TEST_RELEASE_TAG-$target.tar.gz" \
        >"terminal-file-manager-$TEST_RELEASE_TAG-$target.tar.gz.sha256")
done
"$REAL_TAR" -czf "$release_dir/missing.tar.gz" -C "$root/empty" .
TEST_LATEST_URL=https://github.com/seapagan/terminal-file-manager/releases/tag/$TEST_RELEASE_TAG
TEST_RELEASE_DIR=$release_dir
TEST_DOWNLOAD_LOG=$root/downloads.log
TEST_DOWNLOADER_LOG=$root/downloaders.log
TEST_EVENT_LOG=$root/events.log
TEST_TMP_PARENT=$root/installer-tmp
TEST_MKTEMP_COUNTER=$root/mktemp-counter
TEST_MKTEMP_LAST=$root/mktemp-last
TEST_STAGE_COUNTER=$root/stage-counter
TEST_STAGE_LAST=$root/stage-last
export TEST_LATEST_URL TEST_RELEASE_DIR TEST_DOWNLOAD_LOG TEST_DOWNLOADER_LOG
export TEST_EVENT_LOG TEST_TMP_PARENT TEST_MKTEMP_COUNTER TEST_MKTEMP_LAST
export TEST_STAGE_COUNTER TEST_STAGE_LAST

printf '0\n' >"$TEST_MKTEMP_COUNTER"
printf '0\n' >"$TEST_STAGE_COUNTER"
passes=0

fail() {
    printf 'FAILED: %s\n' "$1" >&2
    exit 1
}

pass() { passes=$((passes + 1)); }

TFM_INSTALL_DIR=$root/install
export TFM_INSTALL_DIR

reset_env() {
    rm -rf "${TFM_INSTALL_DIR:-}"
    rm -f "$TEST_DOWNLOAD_LOG" "$TEST_DOWNLOADER_LOG" "$TEST_EVENT_LOG"
    printf '0\n' >"$TEST_MKTEMP_COUNTER"
    printf '0\n' >"$TEST_STAGE_COUNTER"
    unset TFM_VERSION TFM_LIBC XDG_BIN_HOME \
        TEST_OS TEST_ARCH TEST_GLIBC_VERSION TEST_GETCONF_KIND TEST_LDD_KIND \
        TEST_CANDIDATE_EXIT TEST_CANDIDATE_VERSION TEST_STAGE_FAIL \
        TEST_INSTALL_FAIL TEST_MOVE_FAIL TEST_INSTALL_SIGNAL TEST_BIN TEST_TAR_INSPECT_FAIL TEST_CHANGE_FINAL
    TEST_GLIBC_VERSION=2.39
    TEST_LATEST_URL=https://github.com/seapagan/terminal-file-manager/releases/tag/$TEST_RELEASE_TAG
    export TEST_GLIBC_VERSION TEST_LATEST_URL
    TFM_INSTALL_DIR=$root/install
    export TFM_INSTALL_DIR
}

run_install() {
    env PATH="${TEST_BIN:-$test_bin}" /bin/sh ./install.sh "$@" >"$root/output" 2>&1
}

assert_fails() {
    if run_install; then fail "$1"; fi
}

assert_output() { grep -Fq "$1" "$root/output" || fail "missing output: $1"; }

assert_no_output() {
    if grep -Fq "$1" "$root/output"; then fail "unexpected output: $1"; fi
}

assert_downloaded() {
    grep -Fxq "https://github.com/seapagan/terminal-file-manager/releases/download/$TEST_RELEASE_TAG/$1" \
        "$TEST_DOWNLOAD_LOG" || fail "asset was not downloaded: $1"
}

assert_target() {
    assert_target_named "$1"
    assert_downloaded "terminal-file-manager-$TEST_RELEASE_TAG-$1.tar.gz"
}

assert_target_named() {
    test -x "$TFM_INSTALL_DIR/tfm" || fail "tfm was not installed"
}

assert_old_binary() {
    test "$(cat "$TFM_INSTALL_DIR/tfm")" = 'old tfm' || \
        fail 'existing tfm changed after failure'
}

assert_no_binary() {
    test ! -e "$TFM_INSTALL_DIR/tfm" || \
        fail 'failed installation created a final binary'
}

assert_tmp_cleaned() {
    test -z "$(find "$TEST_TMP_PARENT" -mindepth 1 -print)" || \
        fail 'temporary files were not cleaned up'
}

# Keep the complete inherited case sequence separate from approved additions.
. ./tests/installers/unix-cases.sh
. ./tests/installers/unix-security.sh
sh ./tests/installers/macho-header.sh
