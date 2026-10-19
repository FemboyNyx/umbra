# rust_lib_umbra

Flutter FFI plugin that bundles `umbra_core` (`../core`, crate name `umbra_core`).

`cargokit/` is the flutter_rust_bridge 2.13.0 vendored copy, with one local
change in `cmake/cargokit.cmake`: an absolute `manifest_dir` is used as-is so
the crate can live at the repo's `core/` instead of inside `app/`.

When `cargo` is on `PATH` and `core/Cargo.toml` exists, Android, Linux, and
Windows builds compile the real crate. When `cargo` is missing, desktop builds
bundle a stub `libumbra_core` and Android embeds no native library. The app
defaults to `CoreConfig.mock: true`, so the UI still runs.

Generate Dart bindings from `app/`:

```bash
cargo install flutter_rust_bridge_codegen --version 2.13.0 --locked
flutter_rust_bridge_codegen generate
```
