#!/bin/sh
# R1: exercise production parsing with correctly laid-out thin Mach-O headers.
# Header fixtures establish parsing only, never foreign-native execution.
set -eu
macho_root=$(mktemp -d)
trap 'rm -rf "$macho_root"' 0 1 2 3 15
sed '/^main "\$@"$/d' ./install.sh >"$macho_root/functions.sh"
macho_passes=0
cat >"$macho_root/check.sh" <<'CHECK'
#!/bin/sh
. "$1"
validate_architecture "$2"
CHECK

write_macho_header() {
    case "$1" in
        x86_64) cpu='\007\000\000\001' ;;
        aarch64) cpu='\014\000\000\001' ;;
    esac
    printf '\317\372\355\376%b%b%b\000\000\000\000' \
        "$cpu" "$2" "$3" >"$macho_root/header"
}

check_macho_header() {
    env target="$1-apple-darwin" sh "$macho_root/check.sh" \
        "$macho_root/functions.sh" "$macho_root/header" >"$macho_root/output" 2>&1
}

for macho_arch in x86_64 aarch64; do
    # CPU subtype occupies bytes 9-12; MH_EXECUTE occupies bytes 13-16.
    case "$macho_arch" in
        x86_64) subtype='\003\000\000\200'; alternate='\003\000\000\000' ;;
        aarch64) subtype='\000\000\000\000'; alternate='\001\000\000\000' ;;
    esac
    for macho_subtype in "$subtype" "$alternate"; do
        write_macho_header "$macho_arch" "$macho_subtype" '\002\000\000\000'
        check_macho_header "$macho_arch" || {
            cat "$macho_root/output" >&2
            printf 'FAILED: valid %s Mach-O header rejected\n' "$macho_arch" >&2
            exit 1
        }
        macho_passes=$((macho_passes + 1))
    done
    # Give CPU subtype the old mistaken value: it cannot authorize a bad type.
    for macho_type in '\000\000\000\000' '\006\000\000\000' '\010\000\000\000'; do
        write_macho_header "$macho_arch" '\002\000\000\000' "$macho_type"
        if check_macho_header "$macho_arch"; then
            printf 'FAILED: non-executable %s Mach-O type accepted\n' "$macho_arch" >&2
            exit 1
        fi
        macho_passes=$((macho_passes + 1))
    done
    write_macho_header "$macho_arch" "$subtype" '\002\000\000\000'
    other_arch=x86_64
    test "$macho_arch" != x86_64 || other_arch=aarch64
    if check_macho_header "$other_arch"; then
        printf 'FAILED: wrong Mach-O architecture accepted\n' >&2
        exit 1
    fi
    macho_passes=$((macho_passes + 1))
done
printf 'Mach-O installer header tests passed: %s cases.\n' "$macho_passes"
