#!/bin/sh
# Complete inherited Labeldeck case sequence; sourced by unix.sh.

for case_data in 'x86_64:x86_64-unknown-linux-gnu' \
    'amd64:x86_64-unknown-linux-gnu' \
    'aarch64:aarch64-unknown-linux-gnu' \
    'arm64:aarch64-unknown-linux-gnu'; do
    arch=${case_data%%:*}
    target=${case_data#*:}
    reset_env
    TEST_ARCH=$arch
    TEST_GLIBC_VERSION=2.39
    export TEST_ARCH TEST_GLIBC_VERSION
    run_install || fail "$arch target failed"
    assert_target "$target"
    pass
done

# Darwin: target selection without any libc detection.
for case_data in 'x86_64:x86_64-apple-darwin' 'arm64:aarch64-apple-darwin'; do
    arch=${case_data%%:*}
    target=${case_data#*:}
    reset_env
    TEST_OS=Darwin
    TEST_ARCH=$arch
    TEST_GETCONF_KIND=unavailable
    TEST_LDD_KIND=unavailable
    export TEST_OS TEST_ARCH TEST_GETCONF_KIND TEST_LDD_KIND
    run_install || fail "$arch Darwin target failed"
    assert_target "$target"
    grep -Fxq shasum "$TEST_EVENT_LOG" || fail 'Darwin must use shasum'
    if grep -Eq '^(getconf|ldd)$' "$TEST_EVENT_LOG"; then
        fail 'Darwin must not detect libc'
    fi
    pass
done

reset_env
TEST_OS=FreeBSD
export TEST_OS
assert_fails 'unsupported operating system succeeded'
assert_output 'unsupported operating system: FreeBSD'
assert_no_binary
pass

reset_env
TEST_ARCH=ppc64le
export TEST_ARCH
assert_fails 'unsupported architecture succeeded'
assert_output 'unsupported platform: Linux/ppc64le'
pass

for case_data in '2.39:gnu' '2.28:gnu' '2.27:musl' '1.17:musl'; do
    version=${case_data%%:*}
    expected_libc=${case_data#*:}
    reset_env
    TEST_GLIBC_VERSION=$version
    export TEST_GLIBC_VERSION
    run_install || fail "glibc $version selection failed"
    assert_target "x86_64-unknown-linux-$expected_libc"
    if test "$expected_libc" = musl; then
        assert_output 'older than tfm minimum 2.28'
    fi
    pass
done

reset_env
TEST_GETCONF_KIND=unavailable
TEST_LDD_KIND=musl
export TEST_GETCONF_KIND TEST_LDD_KIND
run_install || fail 'musl detection failed'
assert_target x86_64-unknown-linux-musl
pass

reset_env
TEST_GETCONF_KIND=unavailable
TEST_LDD_KIND=glibc
TEST_GLIBC_VERSION=2.39
export TEST_GETCONF_KIND TEST_LDD_KIND TEST_GLIBC_VERSION
run_install || fail 'ldd glibc fallback failed'
assert_target x86_64-unknown-linux-gnu
pass

reset_env
TEST_GETCONF_KIND=unavailable
TEST_LDD_KIND=unavailable
export TEST_GETCONF_KIND TEST_LDD_KIND
assert_fails 'unknown libc succeeded'
assert_output 'unable to detect libc'
assert_output 'TFM_LIBC=gnu or TFM_LIBC=musl'
pass

reset_env
TEST_GETCONF_KIND=malformed
TEST_LDD_KIND=malformed
export TEST_GETCONF_KIND TEST_LDD_KIND
assert_fails 'malformed libc data succeeded'
assert_output 'unable to detect libc'
pass

for libc in gnu musl; do
    reset_env
    TFM_LIBC=$libc
    export TFM_LIBC
    run_install || fail "libc override $libc failed"
    assert_target "x86_64-unknown-linux-$libc"
    pass
done

reset_env
TEST_ARCH=aarch64
TFM_LIBC=musl
export TEST_ARCH TFM_LIBC
run_install || fail 'aarch64 musl target failed'
assert_target aarch64-unknown-linux-musl
pass

reset_env
TFM_LIBC=other
export TFM_LIBC
assert_fails 'invalid libc override succeeded'
assert_output 'TFM_LIBC must be gnu or musl'
pass

reset_env
TFM_LIBC=gnu
TEST_GLIBC_VERSION=2.17
TEST_CANDIDATE_EXIT=127
mkdir -p "$TFM_INSTALL_DIR"
printf 'old tfm\n' >"$TFM_INSTALL_DIR/tfm"
export TFM_LIBC TEST_GLIBC_VERSION TEST_CANDIDATE_EXIT
assert_fails 'incompatible forced GNU candidate succeeded'
assert_old_binary
pass

reset_env
run_install || fail 'target URLs failed'
assert_target x86_64-unknown-linux-gnu
downloads=$(grep -Fc '/releases/download/' "$TEST_DOWNLOAD_LOG" || true)
test "$downloads" -eq 2 || fail 'wrong download count'
pass

reset_env
run_install || fail 'valid checksum failed'
grep -Fxq sha256sum "$TEST_EVENT_LOG" || fail 'sha256sum did not run'
pass

for mode in bad malformed wrong-name external-file; do
    reset_env
    archive=terminal-file-manager-$TEST_RELEASE_TAG-x86_64-unknown-linux-gnu.tar.gz
    sidecar=$release_dir/$archive.sha256
    backup=$root/sidecar-backup
    cp "$sidecar" "$backup"
    case $mode in
        bad) printf '%064d  %s\n' 0 "$archive" >"$sidecar" ;;
        malformed) printf 'not-a-checksum  %s\n' "$archive" >"$sidecar" ;;
        wrong-name) printf '%s  other-file\n' "$(cut -d' ' -f1 "$backup")" >"$sidecar" ;;
        external-file) printf '%s  /etc/passwd\n' "$(cut -d' ' -f1 "$backup")" >"$sidecar" ;;
    esac
    assert_fails "bad checksum ($mode) succeeded"
    assert_output 'checksum verification failed'
    assert_no_binary
    cp "$backup" "$sidecar"
    pass
done

reset_env
TEST_BIN=$no_sha_bin
export TEST_BIN
assert_fails 'missing sha256sum succeeded'
assert_output 'sha256sum is required'
assert_no_binary
pass

reset_env
TEST_CANDIDATE_EXIT=1
mkdir -p "$TFM_INSTALL_DIR"
printf 'old tfm\n' >"$TFM_INSTALL_DIR/tfm"
export TEST_CANDIDATE_EXIT
assert_fails 'candidate non-zero exit succeeded'
assert_old_binary
pass

reset_env
TEST_CANDIDATE_VERSION=9.9.9
export TEST_CANDIDATE_VERSION
assert_fails 'wrong candidate version succeeded'
assert_output "reported version does not match $TEST_RELEASE_TAG"
assert_no_binary
pass

reset_env
bad_archive=$root/bad-archive
mkdir "$bad_archive"
printf '#!/missing/interpreter\n' >"$bad_archive/tfm"
chmod +x "$bad_archive/tfm"
cp "$root/archive/README.md" "$root/archive/LICENSE-MIT" "$root/archive/LICENSE-APACHE" "$bad_archive/"
archive=terminal-file-manager-$TEST_RELEASE_TAG-x86_64-unknown-linux-gnu.tar.gz
"$REAL_TAR" -czf "$release_dir/$archive" -C "$bad_archive" tfm README.md LICENSE-MIT LICENSE-APACHE
(cd "$release_dir" && "$REAL_SHA256SUM" "$archive" >"$archive.sha256")
assert_fails 'unexecutable candidate succeeded'
assert_output 'candidate failed --version validation'
assert_no_binary
"$REAL_TAR" -czf "$release_dir/$archive" -C "$root/archive" tfm README.md LICENSE-MIT LICENSE-APACHE
(cd "$release_dir" && "$REAL_SHA256SUM" "$archive" >"$archive.sha256")
pass

reset_env
TFM_INSTALL_DIR=$root/success-bin
export TFM_INSTALL_DIR
run_install || fail 'successful replacement failed'
test -x "$TFM_INSTALL_DIR/tfm" || fail 'installed binary is not executable'
test "$("$TFM_INSTALL_DIR/tfm" --version)" = "tfm $TEST_RELEASE_VERSION" || \
    fail 'installed candidate is wrong'
test ! -e "$TFM_INSTALL_DIR/README.md" || fail 'README was installed'
test ! -e "$TFM_INSTALL_DIR/LICENSE-MIT" || fail 'licence was installed'
pass

for failure in stage install move; do
    reset_env
    mkdir -p "$TFM_INSTALL_DIR"
    printf 'old tfm\n' >"$TFM_INSTALL_DIR/tfm"
    case $failure in
        stage) TEST_STAGE_FAIL=1 ;;
        install) TEST_INSTALL_FAIL=1 ;;
        move) TEST_MOVE_FAIL=1 ;;
    esac
    export TEST_STAGE_FAIL TEST_INSTALL_FAIL TEST_MOVE_FAIL
    assert_fails "$failure failure succeeded"
    assert_old_binary
    test -z "$(find "$TFM_INSTALL_DIR" -name '.tfm.*' -print)" || \
        fail "$failure failure left a destination staging file"
    pass
done

reset_env
mkdir -p "$TFM_INSTALL_DIR"
printf 'old tfm\n' >"$TFM_INSTALL_DIR/tfm"
TEST_INSTALL_SIGNAL=1
export TEST_INSTALL_SIGNAL
assert_fails 'interrupted install succeeded'
assert_old_binary
test -z "$(find "$TFM_INSTALL_DIR" -name '.tfm.*' -print)" || \
    fail 'interrupt left a destination staging file'
pass

reset_env
mkdir -p "$TFM_INSTALL_DIR"
printf 'old tfm\n' >"$TFM_INSTALL_DIR/tfm"
run_install || fail 'replacement of existing binary failed'
test "$("$TFM_INSTALL_DIR/tfm" --version)" = "tfm $TEST_RELEASE_VERSION" || \
    fail 'existing binary was not replaced'
pass

reset_env
TFM_VERSION=
export TFM_VERSION
run_install || fail 'latest-release lookup failed'
grep -Fxq 'https://github.com/seapagan/terminal-file-manager/releases/latest' "$TEST_DOWNLOAD_LOG" || \
    fail 'GitHub latest-release redirect was not used'
if grep -Fq 'api.github.com' "$TEST_DOWNLOAD_LOG"; then
    fail 'latest lookup used api.github.com'
fi
assert_target x86_64-unknown-linux-gnu
pass

reset_env
TFM_VERSION=$TEST_RELEASE_TAG
export TFM_VERSION
run_install || fail 'explicit version failed'
if grep -Fq '/releases/latest' "$TEST_DOWNLOAD_LOG"; then
    fail 'explicit version queried latest release'
fi
pass

reset_env
TFM_VERSION=
TEST_LATEST_URL=https://github.com/seapagan/terminal-file-manager/releases/tag/not-a-version
export TFM_VERSION TEST_LATEST_URL
assert_fails 'malformed latest-release redirect succeeded'
assert_output 'latest release redirect did not resolve to a usable release tag'
if grep -Fq '/releases/download/' "$TEST_DOWNLOAD_LOG"; then
    fail 'malformed latest redirect attempted an asset download'
fi
pass

for install_state in existing fresh; do
    # Old release ships only the checksum sidecar: the archive itself is
    # missing (pre-layout release).
    old_archive=terminal-file-manager-$TEST_OLD_TAG-x86_64-unknown-linux-gnu.tar.gz
    printf '%s  %s\n' "$(printf 'x%.0s' $(seq 64))" "$old_archive" \
        >"$release_dir/$old_archive.sha256"
    reset_env
    TFM_VERSION=$TEST_OLD_TAG
    export TFM_VERSION
    if test "$install_state" = existing; then
        mkdir -p "$TFM_INSTALL_DIR"
        printf 'old tfm\n' >"$TFM_INSTALL_DIR/tfm"
    fi
    assert_fails 'missing archive asset succeeded'
    assert_output 'could not download required release asset'
    if test "$install_state" = existing; then
        assert_old_binary
    else
        assert_no_binary
    fi
    assert_output 'may predate the current artifact layout'
    rm "$release_dir/$old_archive.sha256"
    pass
done

for install_state in existing fresh; do
    # Old release ships the archive but predates checksum sidecars.
    old_archive=terminal-file-manager-$TEST_OLD_TAG-x86_64-unknown-linux-gnu.tar.gz
    cp "$release_dir/terminal-file-manager-$TEST_RELEASE_TAG-x86_64-unknown-linux-gnu.tar.gz" \
        "$release_dir/$old_archive"
    reset_env
    TFM_VERSION=$TEST_OLD_TAG
    export TFM_VERSION
    if test "$install_state" = existing; then
        mkdir -p "$TFM_INSTALL_DIR"
        printf 'old tfm\n' >"$TFM_INSTALL_DIR/tfm"
    fi
    assert_fails 'missing checksum asset succeeded'
    assert_output 'could not download required checksum asset'
    if test "$install_state" = existing; then
        assert_old_binary
    else
        assert_no_binary
    fi
    assert_output 'may predate checksum-backed installer support'
    rm "$release_dir/$old_archive"
    pass
done

current_archive=terminal-file-manager-$TEST_RELEASE_TAG-x86_64-unknown-linux-gnu.tar.gz
mv "$release_dir/$current_archive" "$root/current-archive"
for install_state in existing fresh; do
    reset_env
    if test "$install_state" = existing; then
        mkdir -p "$TFM_INSTALL_DIR"
        printf 'old tfm\n' >"$TFM_INSTALL_DIR/tfm"
    fi
    assert_fails 'missing current archive succeeded'
    assert_output 'could not download required release asset'
    if test "$install_state" = existing; then
        assert_old_binary
    else
        assert_no_binary
    fi
    pass
done
mv "$root/current-archive" "$release_dir/$current_archive"

mv "$release_dir/$current_archive.sha256" "$root/current-checksum"
for install_state in existing fresh; do
    reset_env
    if test "$install_state" = existing; then
        mkdir -p "$TFM_INSTALL_DIR"
        printf 'old tfm\n' >"$TFM_INSTALL_DIR/tfm"
    fi
    assert_fails 'missing current checksum succeeded'
    assert_output 'could not download required checksum asset'
    if test "$install_state" = existing; then
        assert_old_binary
    else
        assert_no_binary
    fi
    pass
done
mv "$root/current-checksum" "$release_dir/$current_archive.sha256"

reset_env
TFM_VERSION=
export TFM_VERSION
run_install || fail 'curl latest-resolution path failed'
grep -Fxq curl "$TEST_DOWNLOADER_LOG" || fail 'curl was not preferred'
pass

reset_env
TEST_BIN=$wget_bin
export TEST_BIN
TFM_VERSION=
export TFM_VERSION
run_install || fail 'wget latest-resolution fallback failed'
grep -Fxq wget "$TEST_DOWNLOADER_LOG" || fail 'wget did not run'
pass

reset_env
TEST_BIN=$no_download_bin
export TEST_BIN
assert_fails 'missing downloader succeeded'
assert_output 'curl or wget is required'
assert_tmp_cleaned
pass

reset_env
TFM_INSTALL_DIR=$root/'explicit bin'
XDG_BIN_HOME=$root/ignored-xdg
HOME=$root/ignored-home
export TFM_INSTALL_DIR XDG_BIN_HOME HOME
run_install || fail 'explicit install directory failed'
test -x "$TFM_INSTALL_DIR/tfm" || fail 'explicit install directory was ignored'
test ! -e "$XDG_BIN_HOME/tfm" || fail 'XDG overrode explicit directory'
pass

reset_env
XDG_BIN_HOME=$root/'xdg bin'
TFM_INSTALL_DIR=
export XDG_BIN_HOME TFM_INSTALL_DIR
run_install || fail 'XDG install directory failed'
test -x "$XDG_BIN_HOME/tfm" || fail 'XDG directory was ignored'
pass

reset_env
HOME=$root/'default home'
TFM_INSTALL_DIR=
export HOME TFM_INSTALL_DIR
run_install || fail 'default install directory failed'
test -x "$HOME/.local/bin/tfm" || fail 'default directory was ignored'
pass

reset_env
unset TFM_INSTALL_DIR XDG_BIN_HOME HOME
assert_fails 'missing install directory inputs succeeded'
assert_output 'no install directory could be determined; set TFM_INSTALL_DIR'
test ! -s "$TEST_DOWNLOAD_LOG" || \
    fail 'missing install directory inputs attempted a release download'
pass

reset_env
archive=terminal-file-manager-$TEST_RELEASE_TAG-x86_64-unknown-linux-gnu.tar.gz
cp "$release_dir/missing.tar.gz" "$release_dir/$archive"
(cd "$release_dir" && "$REAL_SHA256SUM" "$archive" >"$archive.sha256")
assert_fails 'archive without tfm succeeded'
assert_output 'release archive is missing tfm'
assert_no_binary
"$REAL_TAR" -czf "$release_dir/$archive" -C "$root/archive" tfm README.md LICENSE-MIT LICENSE-APACHE
(cd "$release_dir" && "$REAL_SHA256SUM" "$archive" >"$archive.sha256")
pass

reset_env
TFM_INSTALL_DIR=$root/not-on-path
export TFM_INSTALL_DIR
run_install || fail 'PATH warning setup failed'
assert_output 'add it to PATH'
pass

reset_env
TFM_INSTALL_DIR=$root/'on path'
TEST_BIN="$test_bin:$TFM_INSTALL_DIR"
export TFM_INSTALL_DIR TEST_BIN
run_install || fail 'exact PATH setup failed'
if grep -Fq 'add it to PATH' "$root/output"; then
    fail 'exact PATH component produced a warning'
fi
assert_tmp_cleaned
pass
