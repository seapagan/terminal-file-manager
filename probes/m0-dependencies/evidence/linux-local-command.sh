#!/bin/bash
set -euxo pipefail
export RUSTUP_TOOLCHAIN="$RUST_VERSION"
export CARGO_HOME=/proof/cargo
export RUSTUP_HOME=/proof/rustup
export CARGO_TARGET_DIR=/proof/target
mkdir -p /proof/bootstrap
rustup_init=/proof/bootstrap/rustup-init
curl --proto '=https' --tlsv1.2 -sSf --output "$rustup_init" "https://static.rust-lang.org/rustup/archive/$RUSTUP_VERSION/$TARGET/rustup-init"
printf '%s  %s\n' "$RUSTUP_INIT_SHA256" "$rustup_init" | sha256sum -c -
chmod 755 "$rustup_init"
"$rustup_init" -y --no-modify-path --profile minimal --default-toolchain "$RUST_VERSION"
export PATH="$CARGO_HOME/bin:$PATH"
rustup component add clippy rust-docs
rustup target add "$TARGET"
rustc -vV
cargo -vV
uname -a
cc --version
ld --version
as --version
ldd --version || test "$LIBC" = musl
printf '%s  %s\n' d94748671869250b4b8468c9e96608893d61dbf6b109d9f832fc0f13d61ca7bd probes/m0-dependencies/Cargo.lock | sha256sum -c -
manifest=probes/m0-dependencies/Cargo.toml
cargo check --workspace --all-targets --all-features --locked --target "$TARGET" --manifest-path "$manifest"
cargo clippy --workspace --all-targets --all-features --locked --target "$TARGET" --manifest-path "$manifest" -- -D warnings
cargo test --workspace --all-targets --all-features --locked --target "$TARGET" --manifest-path "$manifest"
cargo test --workspace --doc --all-features --locked --target "$TARGET" --manifest-path "$manifest"
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --all-features --locked --target "$TARGET" --manifest-path "$manifest"
cargo build -vv --release --locked --target "$TARGET" --manifest-path "$manifest"
binary="$CARGO_TARGET_DIR/$TARGET/release/tfm-m0-dependency-probe"
sh scripts/verify-release-binary.sh "$LIBC" "$ARCH" "$binary"
readelf -hW "$binary"
readelf -dW "$binary"
test "$(timeout 10 "$binary")" = 'tfm M0 dependency probe: OK'
stat -c 'size before stripping: %s bytes' "$binary"
strip --strip-all "$binary"
stat -c 'size after stripping: %s bytes' "$binary"
if test "$LIBC" = musl; then verify_kind=static; else verify_kind=gnu; fi
sh scripts/verify-release-binary.sh "$verify_kind" "$ARCH" "$binary"
test "$(timeout 10 "$binary")" = 'tfm M0 dependency probe: OK'
