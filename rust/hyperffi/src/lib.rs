//! JNI bridge: exposes a `hypercore::Session` to the Android host as a long-lived
//! native handle, exchanging JSON strings across the boundary.
//!
//! The body is compiled only for Android targets (where the `jni` crate is available);
//! on other targets this crate builds as an empty cdylib so the workspace stays buildable
//! on the desktop.

#[cfg(target_os = "android")]
mod android;
