/// Native build glue for `umbra_core`.
///
/// The Dart UI does not call into this library. `flutter_rust_bridge` loads
/// `libumbra_core` itself after `flutter_rust_bridge_codegen generate`.
/// Cargokit (or a stub, when `cargo` is absent) produces that library.
library;
