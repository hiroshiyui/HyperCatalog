//! UniFFI bridge: exposes `hypercore::Session` to the Android host as a typed `HyperStack`
//! object (ADR-0012). The Kotlin bindings are generated from `bridge.rs`; there is no
//! hand-written JNI and no JSON on the wire.

mod bridge;

uniffi::setup_scaffolding!();
