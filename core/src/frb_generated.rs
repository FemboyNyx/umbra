// PLACEHOLDER — overwritten by `flutter_rust_bridge_codegen generate`.
//
// It only exists so the crate compiles (and `cargo test` runs) before the app
// side has run codegen. The generated file defines the real `StreamSink<T>`
// with the same `add(&self, T) -> Result<(), _>` method used in `src/api/`.
#![allow(dead_code)]

use std::marker::PhantomData;

/// Stand-in for the codegen-provided `StreamSink<T>`.
pub struct StreamSink<T> {
    _p: PhantomData<fn(T)>,
}

impl<T> StreamSink<T> {
    pub fn add(&self, _value: T) -> Result<(), String> {
        Err("flutter_rust_bridge codegen has not been run".into())
    }
}
