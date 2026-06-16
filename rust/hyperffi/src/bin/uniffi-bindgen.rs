//! Library-mode binding generator for hyperffi (ADR-0012). Run:
//!   cargo run -p hyperffi --bin uniffi-bindgen -- generate --library <.so> --language kotlin ...
fn main() {
    uniffi::uniffi_bindgen_main()
}
