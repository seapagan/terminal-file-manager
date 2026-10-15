#!/usr/bin/env bash
# Characterize the actual pre/post-strip workflow blocks with a fake file tool.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT
passes=0
mkdir "$fixture/bin"
cat > "$fixture/bin/file" <<'EOF'
#!/bin/sh
test "$#" -eq 1 && test "$1" = "target/$TARGET/release/tfm" || exit 2
printf '%s\n' "$TEST_FILE_DESCRIPTION"
exit "$TEST_FILE_STATUS"
EOF
chmod +x "$fixture/bin/file"
export PATH="$fixture/bin:$PATH"
export TARGET=fixture-target
export TEST_FILE_STATUS=0

check() {
    local expected_status=$1
    export EXPECTED_ARCH=$2
    export TEST_FILE_DESCRIPTION=$3
    local status=0
    bash -eu "$fixture/check.sh" > "$fixture/output" 2>&1 || status=$?
    if { test "$expected_status" = pass && test "$status" -ne 0; } ||
       { test "$expected_status" = fail && test "$status" -eq 0; }; then
        cat "$fixture/output" >&2
        echo "error: $step: expected $expected_status for $TEST_FILE_DESCRIPTION" >&2
        exit 1
    fi
    passes=$((passes + 1))
}

for step in 'Verify Mach-O architecture' 'Verify stripped Mach-O architecture'; do
    # Limit extraction to the named step; fail if it stops being a run block.
    sed -n "/^      - name: $step\$/,/^      - name:/p" \
        "$root/.github/workflows/release-rehearsal.yml" |
        sed '1,/run: |/d; /^      - name:/,$d; s/^          //' \
        > "$fixture/check.sh"
    test -s "$fixture/check.sh"
    for arch in x86_64 arm64; do
        check pass "$arch" "target/$TARGET/release/tfm: Mach-O 64-bit executable $arch"
        other_arch=arm64
        if test "$arch" = arm64; then other_arch=x86_64; fi
        check fail "$arch" "target/$TARGET/release/tfm: Mach-O 64-bit executable $other_arch"
        check fail "$arch" 'target/fixture-target/release/tfm: Mach-O 64-bit executable i386'
        check fail "$arch" 'target/fixture-target/release/tfm: ELF 64-bit executable'
        export TEST_FILE_STATUS=1
        check fail "$arch" "target/$TARGET/release/tfm: Mach-O 64-bit executable $arch"
        export TEST_FILE_STATUS=0
    done
done
printf 'Mach-O workflow characterization passed: %s cases.\n' "$passes"
