#!/bin/sh
set -eu

# Build the directory on the container filesystem, including the non-UTF-8 name.
suite=$1/suite
mkdir -p "$suite/child" "$suite/empty" "$suite/denied"
chmod 755 "$1" "$suite" "$suite/child" "$suite/empty"
chmod 000 "$suite/denied"
touch "$suite/child/child-marker"
printf 'fixture\n' > "$suite/plain-file"
chmod 640 "$suite/plain-file"
TZ=UTC0 touch -t 202311142213.20 "$suite/plain-file"
touch "$suite/missing-metadata"
if [ "${2:-raw}" = raw ]; then
    touch "$suite/$(printf 'byte-\377')"
else
    touch "$suite/byte-界"
fi
ln -s child "$suite/linked-dir"
ln -s plain-file "$suite/linked-file"
ln -s missing "$suite/broken-link"
