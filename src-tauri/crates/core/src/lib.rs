//! Pure, unit-tested logic: the Rust port of the Swift core module. No I/O beyond the meeting store.
pub mod ask;
pub mod detector_logic;
pub mod error;
pub mod export;
pub mod folders;
pub mod models;
pub mod oauth;
pub mod search;
pub mod sharing;
pub mod store;
pub mod summary;
pub mod transcript;
pub mod windower;

pub use error::{Error, Result};
