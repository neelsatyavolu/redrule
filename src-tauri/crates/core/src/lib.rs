//! Pure, unit-tested logic: the Rust port of the Swift core module. No I/O beyond the meeting store.
pub mod detector_logic;
pub mod error;
pub mod models;
pub mod oauth;
pub mod store;
pub mod summary;
pub mod transcript;
pub mod windower;

pub use error::{Error, Result};
