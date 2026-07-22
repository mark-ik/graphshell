//! Graphshell's G0 facade.
//!
//! This crate intentionally has no renderer or carrier. Its job at this stage
//! is to make the portable protocol boundary importable as one package.

pub use graphshell_client as client;
pub use graphshell_endpoint as endpoint;
pub use graphshell_protocol as protocol;
