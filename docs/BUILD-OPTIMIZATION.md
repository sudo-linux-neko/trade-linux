# Build optimization (this machine)

Target: 13th Gen Intel Core i5-1335U (Raptor Lake-U), AVX2+FMA, no AVX-512.

`Cargo.toml`:

```toml
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true
```

```sh
RUSTFLAGS="-C target-cpu=native" cargo build --release
```
