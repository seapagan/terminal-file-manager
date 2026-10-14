#!/bin/sh
# tfm installer for Linux and macOS.
#
# Downloads the correct release archive for this platform, verifies its
# SHA-256, executes the candidate binary's --version as a smoke test,
# and only then replaces any existing installation atomically.
set -eu

die() {
    printf 'error: %s\n' "$1" >&2
    return 1
}

detect_arch() {
    os=$1
    arch=$(uname -m)
    case "$os:$arch" in
        Linux:x86_64|Linux:amd64|Darwin:x86_64) printf '%s\n' x86_64 ;;
        Linux:aarch64|Linux:arm64|Darwin:arm64) printf '%s\n' aarch64 ;;
        *) die "unsupported platform: $os/$arch" ;;
    esac
}

glibc_is_supported() {
    version=$1
    major=${version%%.*}
    minor=${version#*.}
    test "$major" -gt 2 || { test "$major" -eq 2 && test "$minor" -ge 28; }
}

detect_libc() {
    if command -v getconf >/dev/null 2>&1; then
        getconf_output=$(getconf GNU_LIBC_VERSION 2>/dev/null || :)
        version=$(printf '%s\n' "$getconf_output" | sed -n \
            's/^glibc \([0-9][0-9]*\.[0-9][0-9]*\)$/\1/p')
        if test -n "$version"; then
            select_glibc "$version"
            return
        fi
    fi

    if command -v ldd >/dev/null 2>&1; then
        ldd_output=$(ldd --version 2>&1 || :)
        case "$ldd_output" in
            *musl*|*Musl*) printf '%s\n' musl; return ;;
            *'GNU libc'*|*GLIBC*|*'GNU C Library'*)
                version=$(printf '%s\n' "$ldd_output" | sed -n \
                    '1s/.* \([0-9][0-9]*\.[0-9][0-9]*\)$/\1/p')
                if test -n "$version"; then
                    select_glibc "$version"
                    return
                fi
                ;;
        esac
    fi

    die 'unable to detect libc; set TFM_LIBC=gnu or TFM_LIBC=musl'
}

select_glibc() {
    version=$1
    if glibc_is_supported "$version"; then
        printf '%s\n' gnu
    else
        printf 'Detected glibc %s, older than tfm minimum 2.28; using static musl build.\n' \
            "$version" >&2
        printf '%s\n' musl
    fi
}

select_libc() {
    case "${TFM_LIBC:-}" in
        gnu|musl) printf '%s\n' "$TFM_LIBC" ;;
        '') detect_libc ;;
        *) die 'TFM_LIBC must be gnu or musl' ;;
    esac
}

detect_target() {
    os=$(uname -s)
    case "$os" in
        Linux)
            arch=$(detect_arch "$os")
            libc=$(select_libc)
            printf '%s\n' "$arch-unknown-linux-$libc"
            ;;
        Darwin)
            arch=$(detect_arch "$os")
            printf '%s\n' "$arch-apple-darwin"
            ;;
        *)
            die "unsupported operating system: $os (use install.ps1 on Windows)"
            ;;
    esac
}

download() {
    if command -v curl >/dev/null 2>&1; then
        curl --proto '=https' --proto-redir '=https' -fsSL "$1" -o "$2"
    elif command -v wget >/dev/null 2>&1; then
        wget --https-only -qO "$2" "$1"
    else
        die 'curl or wget is required'
    fi
}

tmp_dir=
stage_file=

cleanup() {
    if test -n "$stage_file"; then
        rm -f "$stage_file"
    fi
    if test -n "$tmp_dir"; then
        rm -rf "$tmp_dir"
    fi
}

resolve_version() {
    version=${TFM_VERSION:-}
    if test -z "$version"; then
        latest_url=https://github.com/seapagan/terminal-file-manager/releases/latest
        if command -v curl >/dev/null 2>&1; then
            release_url=$(curl --proto '=https' --proto-redir '=https' -fsSL -o /dev/null -w '%{url_effective}\n' "$latest_url") || \
                die 'could not resolve latest release redirect'
        elif command -v wget >/dev/null 2>&1; then
            headers=$tmp_dir/latest.headers
            wget --https-only -q --spider --server-response "$latest_url" 2>"$headers" || \
                die 'could not resolve latest release redirect'
            release_url=$(awk '
                tolower($1) == "location:" { url = $2 }
                END { sub(/\r$/, "", url); print url }
            ' "$headers")
        else
            die 'curl or wget is required'
        fi
        version=$(printf '%s\n' "$release_url" | sed -n \
            's#^https://github\.com/seapagan/terminal-file-manager/releases/tag/\(v[0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*\)$#\1#p')
        test -n "$version" || \
            die 'latest release redirect did not resolve to a usable release tag'
    fi
    case "$version" in ''|*[!v0-9.]*) die 'version must be a stable vX.Y.Z release tag' ;; esac
    printf '%s\n' "$version" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+$' ||
        die 'version must be a stable vX.Y.Z release tag'
    printf '%s\n' "$version"
}

checksum_tool() {
    os=$(uname -s)
    case "$os" in
        Linux)
            command -v sha256sum >/dev/null 2>&1 || die 'sha256sum is required'
            printf '%s\n' sha256sum
            ;;
        Darwin)
            command -v shasum >/dev/null 2>&1 || die 'shasum is required'
            printf '%s\n' 'shasum -a 256'
            ;;
        *) die "unsupported operating system: $os" ;;
    esac
}

fetch_and_verify_release() {
    version=$1
    target=$2
    explicit_version=$3
    checksum=$(checksum_tool)
    asset="terminal-file-manager-$version-$target.tar.gz"
    base_url="https://github.com/seapagan/terminal-file-manager/releases/download/$version"
    if ! download "$base_url/$asset" "$tmp_dir/$asset"; then
        printf 'error: could not download required release asset: %s\n' "$asset" >&2
        if test "$explicit_version" = 1; then
            printf '%s\n' \
                'If this is an older release, it may predate the current artifact layout.' \
                "See https://github.com/seapagan/terminal-file-manager/releases/tag/$version for the files provided by that release." >&2
        fi
        return 1
    fi
    if ! download "$base_url/$asset.sha256" "$tmp_dir/$asset.sha256"; then
        printf 'error: could not download required checksum asset: %s.sha256\n' "$asset" >&2
        if test "$explicit_version" = 1; then
            printf '%s\n' \
                'If this is an older release, it may predate checksum-backed installer support.' \
                "See https://github.com/seapagan/terminal-file-manager/releases/tag/$version for the files provided by that release." >&2
        fi
        return 1
    fi

    if ! awk -v expected="$asset" '
        NR == 1 && NF == 2 && length($1) == 64 &&
            $1 !~ /[^0-9a-fA-F]/ && $2 == expected { valid = 1 }
        END { exit !(NR == 1 && valid) }
    ' "$tmp_dir/$asset.sha256"; then
        die 'checksum verification failed'
    fi
    if ! (cd "$tmp_dir" && $checksum -c "$asset.sha256"); then
        die 'checksum verification failed'
    fi

    inspect_archive "$tmp_dir/$asset"
    mkdir "$tmp_dir/extracted"
    tar -xzf "$tmp_dir/$asset" -C "$tmp_dir/extracted"

}

# GNU and BSD tar both expose effective member names and type characters.
# Keep inspection commands separate: a failing tar must never be masked by awk.
inspect_archive() {
    LC_ALL=C tar -tzf "$1" >"$tmp_dir/names" || die 'archive inspection failed'
    LC_ALL=C tar -tvzf "$1" >"$tmp_dir/types" || die 'archive inspection failed'
    grep -Fxq tfm "$tmp_dir/names" || die 'release archive is missing tfm'
    awk '
        $0 != "tfm" && $0 != "LICENSE-MIT" && $0 != "LICENSE-APACHE" &&
            $0 != "README.md" { bad = 1 }
        { if (seen[$0]++) bad = 1 }
        END { exit !(NR == 4 && !bad) }
    ' "$tmp_dir/names" || die 'unsafe archive members'
    awk 'substr($0, 1, 1) != "-" { bad = 1 }
        END { exit !(NR == 4 && !bad) }' "$tmp_dir/types" ||
        die 'unsafe archive member types'
}

# Read only the fixed 20-byte header. No build-time reader or runtime required.
validate_architecture() {
    header=$(od -An -tu1 -N20 "$1") || die 'could not read candidate header'
    printf '%s\n' "$header" | awk -v target="$target" '
        { for (i = 1; i <= NF; i++) b[++n] = $i }
        END {
            if (n != 20) exit 1
            if (target ~ /linux/) {
                valid = b[1] == 127 && b[2] == 69 && b[3] == 76 && b[4] == 70 &&
                    b[5] == 2 && b[6] == 1 && b[7] == 1 &&
                    (b[17] == 2 || b[17] == 3) && b[18] == 0 && b[20] == 0
                machine = target ~ /^x86_64/ ? 62 : 183
                valid = valid && b[19] == machine
            } else {
                machine = target ~ /^x86_64/ ? 7 : 12
                valid = b[1] == 207 && b[2] == 250 && b[3] == 237 && b[4] == 254 &&
                    b[5] == machine && b[6] == 0 && b[7] == 0 && b[8] == 1 &&
                    b[13] == 2 && b[14] == 0 && b[15] == 0 && b[16] == 0
            }
            exit !valid
        }
    ' || die 'invalid candidate executable format or architecture'
}

validate_candidate() {
    version=$1
    candidate=$tmp_dir/extracted/tfm
    test -f "$candidate" && test ! -L "$candidate" || die 'release archive is missing tfm'
    validate_architecture "$candidate"
    chmod 755 "$candidate" || die 'release tfm is not executable'
    # POSIX sh has no portable deadline facility. Validation is synchronous;
    # native release/installer acceptance must prove prompt --version termination.
    if ! "$candidate" --version >"$tmp_dir/version"; then
        die 'candidate failed --version validation'
    fi
    printf 'tfm %s\n' "${version#v}" >"$tmp_dir/expected-version"
    cmp -s "$tmp_dir/version" "$tmp_dir/expected-version" ||
        die "reported version does not match $version: $(cat "$tmp_dir/version")"
}

# Walk each lexical component before creating anything. Reject links rather
# than canonicalizing through them. Rechecking narrows races; it cannot remove
# every race between shell pathname operations and rename.
check_directory() {
    case "$1" in /*) remaining=$1 ;; *) remaining=$(pwd -P)/$1 ;; esac
    checked_dir=
    while test -n "$remaining"; do
        remaining=${remaining#/}
        component=${remaining%%/*}
        if test "$remaining" = "$component"; then remaining=; else remaining=${remaining#*/}; fi
        case "$component" in ''|.) continue ;; esac
        checked_dir=$checked_dir/$component
        test ! -L "$checked_dir" || die "linked destination ancestor: $checked_dir"
        if test -e "$checked_dir"; then
            test -d "$checked_dir" || die "non-directory destination ancestor: $checked_dir"
            test -x "$checked_dir" || die "inaccessible destination ancestor: $checked_dir"
        fi
    done
}

final_identity() {
    final=$1/tfm
    test ! -L "$final" || die 'unsafe final destination: symlink'
    if test -e "$final"; then
        test -f "$final" || die 'unsafe final destination: not a regular file'
        ls -di "$final" && cksum <"$final"
    else
        printf 'absent\n'
    fi
}

replace_candidate() {
    install_dir=$1
    check_directory "$install_dir"
    before=$(final_identity "$install_dir")
    mkdir -p "$install_dir" || die 'could not create installation directory'
    check_directory "$install_dir"
    stage_file=$(mktemp "$install_dir/.tfm.XXXXXX") ||
        die 'could not create destination staging file'
    install -m 755 "$candidate" "$stage_file" ||
        die 'could not stage tfm in the installation directory'
    check_directory "$install_dir"
    after=$(final_identity "$install_dir")
    test "$before" = "$after" || die 'final destination changed during installation'
    mv -f "$stage_file" "$install_dir/tfm" || die 'could not atomically replace tfm'
    test -f "$install_dir/tfm" && test ! -L "$install_dir/tfm" ||
        die 'final destination changed during replacement'
    stage_file=
}

main() {
    if test "$#" -eq 1 && test "$1" = --help; then
        printf '%s\n' 'Usage: install.sh [--help]' \
            'Install tfm for Linux or macOS.' \
            'Configure TFM_VERSION, TFM_INSTALL_DIR and TFM_LIBC through the environment.'
        return
    fi
    test "$#" -eq 0 || die 'unsupported installer arguments; use --help'
    target=$(detect_target)
    if test -n "${TFM_INSTALL_DIR:-}"; then
        install_dir=$TFM_INSTALL_DIR
    elif test -n "${XDG_BIN_HOME:-}"; then
        install_dir=$XDG_BIN_HOME
    elif test -n "${HOME:-}"; then
        install_dir=$HOME/.local/bin
    else
        die 'no install directory could be determined; set TFM_INSTALL_DIR'
    fi
    explicit_version=0
    if test -n "${TFM_VERSION:-}"; then explicit_version=1; fi

    tmp_dir=$(mktemp -d)
    trap cleanup 0
    trap 'exit 1' 1 2 3 15

    version=$(resolve_version)
    fetch_and_verify_release "$version" "$target" "$explicit_version"
    validate_candidate "$version"

    replace_candidate "$install_dir"

    printf 'Installed tfm %s to %s.\n' "$version" "$install_dir"
    case ":${PATH:-}:" in
        *":$install_dir:"*) ;;
        *)
            printf 'warning: %s is not on PATH; add it to PATH to use tfm.\n' \
                "$install_dir" >&2
            ;;
    esac
}

main "$@"
