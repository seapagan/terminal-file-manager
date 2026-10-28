#!/bin/sh
# Approved D2-D5/D7 security additions; sourced by unix.sh.

# D2: valid-checksum archives must be rejected before candidate execution.
archive_name=terminal-file-manager-$TEST_RELEASE_TAG-x86_64-unknown-linux-gnu.tar.gz
cp "$release_dir/$archive_name" "$root/good.tar.gz"
for attack in extra nested duplicate missing-notices renamed-notices duplicate-notices symlink hardlink fifo; do
    reset_env
    mkdir -p "$TFM_INSTALL_DIR"
    printf 'old tfm\n' >"$TFM_INSTALL_DIR/tfm"
    set -- tfm LICENSE-MIT LICENSE-APACHE THIRD_PARTY_NOTICES.md README.md
    case "$attack" in
        extra) printf 'extra\n' >"$root/archive/extra"; set -- "$@" extra ;;
        nested) mkdir -p "$root/archive/nested"; cp "$root/archive/tfm" "$root/archive/nested/tfm"; set -- nested/tfm LICENSE-MIT LICENSE-APACHE THIRD_PARTY_NOTICES.md README.md ;;
        duplicate) set -- "$@" tfm ;;
        missing-notices) set -- tfm LICENSE-MIT LICENSE-APACHE README.md ;;
        renamed-notices)
            cp "$root/archive/THIRD_PARTY_NOTICES.md" "$root/archive/NOTICES.md"
            set -- tfm LICENSE-MIT LICENSE-APACHE NOTICES.md README.md ;;
        duplicate-notices) set -- "$@" THIRD_PARTY_NOTICES.md ;;
        symlink) mv "$root/archive/tfm" "$root/original"; ln -s "$root/outside" "$root/archive/tfm"; : ;;
        hardlink) mv "$root/archive/tfm" "$root/original"; ln "$root/original" "$root/archive/tfm"; set -- LICENSE-MIT LICENSE-APACHE THIRD_PARTY_NOTICES.md README.md tfm tfm ;;
        fifo) mv "$root/archive/tfm" "$root/original"; mkfifo "$root/archive/tfm"; : ;;
    esac
    printf 'outside sentinel\n' >"$root/outside"
    # Deliberate member expansion, confined to the owned fixture root.
    "$REAL_TAR" -czf "$release_dir/$archive_name" -C "$root/archive" "$@"
    (cd "$release_dir" && "$REAL_SHA256SUM" "$archive_name" >"$archive_name.sha256")
    assert_fails "unsafe archive $attack accepted"
    assert_old_binary
    test "$(cat "$root/outside")" = 'outside sentinel' || fail 'outside sentinel changed'
    if grep -Fxq candidate "$TEST_EVENT_LOG"; then fail 'unsafe archive executed candidate'; fi
    assert_tmp_cleaned
    case "$attack" in symlink|hardlink|fifo) rm "$root/archive/tfm"; mv "$root/original" "$root/archive/tfm" ;; esac
    pass
done
cp "$root/good.tar.gz" "$release_dir/$archive_name"
(cd "$release_dir" && "$REAL_SHA256SUM" "$archive_name" >"$archive_name.sha256")

# D3: links and non-regular final objects cannot redirect installation.
for attack in root-link ancestor-link final-link dangling directory ancestor-file; do
    reset_env
    mkdir -p "$root/destination-outside" "$TFM_INSTALL_DIR"
    printf 'outside sentinel\n' >"$root/destination-outside/tfm"
    case "$attack" in
        root-link) rm -rf "$TFM_INSTALL_DIR"; ln -s "$root/destination-outside" "$TFM_INSTALL_DIR" ;;
        ancestor-link) rm -rf "$TFM_INSTALL_DIR"; ln -s "$root/destination-outside" "$TFM_INSTALL_DIR"; TFM_INSTALL_DIR=$TFM_INSTALL_DIR/child ;;
        final-link) ln -s "$root/destination-outside/tfm" "$TFM_INSTALL_DIR/tfm" ;;
        dangling) ln -s "$root/missing" "$TFM_INSTALL_DIR/tfm" ;;
        directory) mkdir "$TFM_INSTALL_DIR/tfm" ;;
        ancestor-file) printf 'file\n' >"$TFM_INSTALL_DIR/file"; TFM_INSTALL_DIR=$TFM_INSTALL_DIR/file/child ;;
    esac
    export TFM_INSTALL_DIR
    assert_fails "unsafe destination $attack accepted"
    test "$(cat "$root/destination-outside/tfm")" = 'outside sentinel' || fail 'destination sentinel changed'
    # Reset only the owned root, never a link target.
    TFM_INSTALL_DIR=$root/install
    export TFM_INSTALL_DIR
    pass
done

# D5: reject unstable overrides before fetching anything.
for tag in v1.2 v1.2.3-rc.1 1.2.3 'v1.2.3/extra' 'v1.2.3
more'; do
    reset_env
    TFM_VERSION=$tag
    export TFM_VERSION
    assert_fails 'invalid stable tag accepted'
    test ! -s "$TEST_DOWNLOAD_LOG" || fail 'invalid tag downloaded assets'
    pass
done

# D4/Keyhold: validation precedes tar, header inspection and execution in
# both fresh and replacement cases. Explicit libc works without detectors.
for installed in no yes; do
    reset_env
    if test "$installed" = yes; then mkdir -p "$TFM_INSTALL_DIR"; printf 'old tfm\n' >"$TFM_INSTALL_DIR/tfm"; fi
    cp "$release_dir/$archive_name.sha256" "$root/good.sha256"
    printf '%064d  %s\n' 0 "$archive_name" >"$release_dir/$archive_name.sha256"
    assert_fails 'checksum mismatch accepted'
    if grep -Eq '^(tar|candidate|install|mv)$' "$TEST_EVENT_LOG"; then fail 'checksum failure reached extraction/execution'; fi
    if test "$installed" = yes; then assert_old_binary; else assert_no_binary; fi
    cp "$root/good.sha256" "$release_dir/$archive_name.sha256"
    assert_tmp_cleaned
    pass
 done
reset_env
TFM_LIBC=gnu
TEST_GETCONF_KIND=unavailable
TEST_LDD_KIND=unavailable
export TFM_LIBC TEST_GETCONF_KIND TEST_LDD_KIND
run_install || fail 'explicit libc needs detectors'
pass

# D2: absolute, traversal and alias names remain unsafe with valid hashes.
for member in ./tfm ../outside "$root/outside"; do
    reset_env
    mkdir -p "$TFM_INSTALL_DIR"
    printf 'old tfm\n' >"$TFM_INSTALL_DIR/tfm"
    printf 'outside sentinel\n' >"$root/outside"
    "$REAL_TAR" -czPf "$release_dir/$archive_name" -C "$root/archive" \
        "$member" LICENSE-MIT LICENSE-APACHE THIRD_PARTY_NOTICES.md README.md
    (cd "$release_dir" && "$REAL_SHA256SUM" "$archive_name" >"$archive_name.sha256")
    assert_fails 'unsafe archive path accepted'
    assert_old_binary
    test "$(cat "$root/outside")" = 'outside sentinel' || fail 'outside path changed'
    if grep -Fxq candidate "$TEST_EVENT_LOG"; then fail 'unsafe path executed candidate'; fi
    pass
done
cp "$root/good.tar.gz" "$release_dir/$archive_name"
(cd "$release_dir" && "$REAL_SHA256SUM" "$archive_name" >"$archive_name.sha256")

# Failed name/type inspection must be fatal, not hidden by a pipeline.
for inspection in names types; do
    reset_env
    TEST_TAR_INSPECT_FAIL=$inspection
    export TEST_TAR_INSPECT_FAIL
    mkdir -p "$TFM_INSTALL_DIR"
    printf 'old tfm\n' >"$TFM_INSTALL_DIR/tfm"
    assert_fails 'failed tar inspection accepted'
    assert_old_binary
    if grep -Fxq candidate "$TEST_EVENT_LOG"; then fail 'failed inspection executed candidate'; fi
    pass
done
reset_env
mkdir -p "$TFM_INSTALL_DIR"
printf 'old tfm\n' >"$TFM_INSTALL_DIR/tfm"
TEST_CHANGE_FINAL=1
export TEST_CHANGE_FINAL
assert_fails 'changed final destination accepted'
test "$(cat "$TFM_INSTALL_DIR/tfm")" = 'external replacement' || fail 'external final object overwritten'
assert_tmp_cleaned
pass

# Build valid-checksum special tar headers without requiring device privileges.
for member_type in 1 2 3 4 6; do
    reset_env
    "$REAL_TAR" --format=ustar -cf "$root/special.tar" -C "$root/archive" \
        tfm LICENSE-MIT LICENSE-APACHE THIRD_PARTY_NOTICES.md README.md
    printf '%s' "$member_type" | dd of="$root/special.tar" bs=1 seek=156 conv=notrunc 2>/dev/null
    printf '        ' | dd of="$root/special.tar" bs=1 seek=148 conv=notrunc 2>/dev/null
    sum=$(od -An -tu1 -N512 "$root/special.tar" | awk '{ for (i=1; i<=NF; i++) sum += $i } END { print sum }')
    printf '%06o\000 ' "$sum" | dd of="$root/special.tar" bs=1 seek=148 conv=notrunc 2>/dev/null
    gzip -c "$root/special.tar" >"$release_dir/$archive_name"
    (cd "$release_dir" && "$REAL_SHA256SUM" "$archive_name" >"$archive_name.sha256")
    assert_fails 'special archive member accepted'
    if grep -Fxq candidate "$TEST_EVENT_LOG"; then fail 'special archive executed candidate'; fi
    assert_no_binary
    assert_tmp_cleaned
    pass
done
cp "$root/good.tar.gz" "$release_dir/$archive_name"
(cd "$release_dir" && "$REAL_SHA256SUM" "$archive_name" >"$archive_name.sha256")

# Real native ELF/Mach-O fixtures, distinct from the inherited shell mocks.
# This proves consumer parsing and execution locally; it is not release-asset
# or foreign-native proof. rustc is a test prerequisite, never a consumer one.
command -v rustc >/dev/null 2>&1 || fail 'rustc is required for native installer fixtures'
rustc -o "$root/native-tfm" - <<'EOF'
fn main() {
    use std::io::Write;
    let path = std::env::var("TEST_EVENT_LOG").unwrap();
    let mut log = std::fs::OpenOptions::new().append(true).create(true).open(path).unwrap();
    writeln!(log, "native-candidate").unwrap();
    let version = std::env::var("TEST_CANDIDATE_VERSION").unwrap_or("99.88.77".into());
    let code = std::env::var("TEST_CANDIDATE_EXIT").unwrap_or("0".into());
    println!("tfm {version}");
    std::process::exit(code.parse().unwrap());
}
EOF
real_od=$(command -v od)
rm "$test_bin/od"
ln -s "$real_od" "$test_bin/od"
cp "$root/archive/tfm" "$root/script-fixture"
cp "$root/native-tfm" "$root/archive/tfm"
native_os=$(uname -s)
native_arch=$(uname -m)
case "$native_os:$native_arch" in
    Linux:x86_64) native_target=x86_64-unknown-linux-gnu ;;
    Linux:aarch64) native_target=aarch64-unknown-linux-gnu ;;
    Darwin:x86_64) native_target=x86_64-apple-darwin ;;
    Darwin:arm64) native_target=aarch64-apple-darwin ;;
    *) fail 'unsupported native fixture host' ;;
esac
native_asset=terminal-file-manager-$TEST_RELEASE_TAG-$native_target.tar.gz
for mode in fresh replacement wrong-version nonzero script wrong-machine truncated; do
    reset_env
    TEST_OS=$native_os
    TEST_ARCH=$native_arch
    export TEST_OS TEST_ARCH
    cp "$root/native-tfm" "$root/archive/tfm"
    case "$mode" in
        wrong-version) TEST_CANDIDATE_VERSION=1.2.3; export TEST_CANDIDATE_VERSION ;;
        nonzero) TEST_CANDIDATE_EXIT=3; export TEST_CANDIDATE_EXIT ;;
        script) cp "$root/script-fixture" "$root/archive/tfm" ;;
        wrong-machine)
            offset=18; test "$native_os" != Darwin || offset=4
            printf '\001' | dd of="$root/archive/tfm" bs=1 seek="$offset" conv=notrunc 2>/dev/null ;;
        truncated) printf '\177ELF' >"$root/archive/tfm" ;;
    esac
    if test "$mode" != fresh; then mkdir -p "$TFM_INSTALL_DIR"; printf 'old tfm\n' >"$TFM_INSTALL_DIR/tfm"; fi
    "$REAL_TAR" -czf "$release_dir/$native_asset" -C "$root/archive" tfm LICENSE-MIT LICENSE-APACHE THIRD_PARTY_NOTICES.md README.md
    (cd "$release_dir" && "$REAL_SHA256SUM" "$native_asset" >"$native_asset.sha256")
    case "$mode" in
        fresh|replacement)
            started=$(date +%s)
            run_install || fail "native $mode failed"
            elapsed=$(($(date +%s) - started))
            test "$elapsed" -le 10 || fail 'native fixture version validation was not prompt'
            cmp -s "$TFM_INSTALL_DIR/tfm" "$root/native-tfm" || fail 'native installed bytes differ'
            ;;
        *)
            assert_fails "native $mode accepted"
            assert_old_binary
            case "$mode" in script|wrong-machine|truncated)
                if grep -Eq '^(candidate|native-candidate)$' "$TEST_EVENT_LOG"; then fail 'invalid header executed candidate'; fi ;;
            esac
            ;;
    esac
    assert_tmp_cleaned
    pass
done

printf 'Unix installer tests passed: %s cases.\n' "$passes"
