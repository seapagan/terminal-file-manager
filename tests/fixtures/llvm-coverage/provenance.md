# Real LLVM exports

These compact fixtures come from executed instrumented programs, exported with
Rust 1.99.0's LLVM 23.1.1 tools (JSON 3.1.0). Only the checkout prefix in source
paths was removed, leaving repository-relative `src/` paths. The fixture programs
do not form part of TFM's production code or coverage instrumentation.

`overlapping` uses `rustc -C instrument-coverage` with this three-line program:

```rust
fn generic<T: Into<u64>>(x: T) -> u64 { x.into() }
fn nested() -> u64 { let f = |x| { if x { 1 } else { 2 } }; let g = |x| { if x { 3 } else { 4 } }; f(true) + g(false) }
fn main() { assert_eq!(generic(1u8), 1); assert_eq!(generic(2u16), 2); assert_eq!(nested(), 5); }
```

LLVM reports six instantiations and five source functions. Its LCOV contains
six `FN:` records and three unique `DA` lines.

`branches` uses Clang 21.1.8 with `-fprofile-instr-generate -fcoverage-mapping`:

```cpp
#define INNER(x) ((x) ? 1 : 2)
#define OUTER(x) INNER(x)
template<class T> int choose(T x) { return x ? OUTER(x) : 3; }
int main() { return choose(1) + choose(0L) == 4 ? 0 : 1; }
```

Clang 21's `llvm-profdata merge -sparse` produces the indexed profile; LLVM 23's
`llvm-cov export -format=text` and `-format=lcov` produce the paired exports.
The nested macro uses multiple file IDs for the same source path. LLVM's default
unified branch export emits eight `BRDA` records, with five hit; the native
function-group summary reports six branches, with three hit. The test checks both
the native summary and the combined record-based summary without fabricating
JSON companions.
