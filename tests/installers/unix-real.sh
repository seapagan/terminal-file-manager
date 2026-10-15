#!/bin/sh
# Real archive acceptance: only the downloader is replaced, never target,
# header, extraction, version or destination operations.
set -eu
command -v perl >/dev/null || { echo 'real-archive tests require platform Perl' >&2; exit 1; }
test "$#" = 4 || { echo 'usage: --real-archive ARCHIVE TARGET VERSION BINARY' >&2; exit 1; }
archive=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
target=$2
version=$3
binary=$(cd "$(dirname "$4")" && pwd)/$(basename "$4")
repo=$(cd "$(dirname "$0")/../.." && pwd)
root=$(mktemp -d)
root=$(cd "$root" && pwd -P)
trap 'rm -rf "$root"' 0 1 2 3 15
mkdir "$root/bin" "$root/mirror" "$root/tmp"
cp "$archive" "$archive.sha256" "$root/mirror/"
name=${archive##*/}
test "$name" = "terminal-file-manager-v$version-$target.tar.gz"
export TFM_VERSION="v$version" TFM_INSTALL_DIR="$root/install" TMPDIR="$root/tmp"
case "$target" in
    *-linux-gnu) TFM_LIBC=gnu; export TFM_LIBC ;;
    *-linux-musl) TFM_LIBC=musl; export TFM_LIBC ;;
    *-apple-darwin) unset TFM_LIBC ;;
    *) echo 'unsupported real-archive target' >&2; exit 1 ;;
esac
TFM_REAL_MIRROR=$root/mirror
export TFM_REAL_MIRROR
cat >"$root/bin/curl" <<'EOF'
#!/bin/sh
set -eu
destination=
url=
while test "$#" -gt 0; do
    case "$1" in
        -o) destination=$2; shift 2 ;;
        --proto|--proto-redir) shift 2 ;;
        -*) shift ;;
        *) url=$1; shift ;;
    esac
done
test -n "$destination"
case "$url" in
    "https://github.com/seapagan/terminal-file-manager/releases/download/$TFM_VERSION/"*)
        cp "$TFM_REAL_MIRROR/${url##*/}" "$destination" ;;
    *) exit 1 ;;
esac
EOF
chmod +x "$root/bin/curl"
PATH=$root/bin:$PATH
export PATH
for installation in fresh reinstall; do
    sh "$repo/install.sh" >"$root/output" 2>&1 || { cat "$root/output"; exit 1; }
    cmp "$binary" "$TFM_INSTALL_DIR/tfm"
    output=$(perl -e 'alarm 10; exec @ARGV or die "could not execute version process"' \
        "$TFM_INSTALL_DIR/tfm" --version)
    test "$output" = "tfm $version"
    test -z "$(find "$root/tmp" -mindepth 1 -print)"
    echo "Real archive $target: $installation passed"
done
printf 'corrupt\n' >>"$root/mirror/$name"
if sh "$repo/install.sh" >"$root/output" 2>&1; then
    echo 'corrupt real archive was installed' >&2; exit 1
fi
cmp "$binary" "$TFM_INSTALL_DIR/tfm"
test -z "$(find "$root/tmp" -mindepth 1 -print)"
test -z "$(find "$TFM_INSTALL_DIR" -name '.tfm.*' -print)"
echo "Real archive $target: corruption rejected; old bytes preserved"
