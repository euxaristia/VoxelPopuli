//! Bedrock world persistence and interchange.
mod archive;
mod blocks;
#[cfg(not(target_arch = "wasm32"))]
pub mod cli;
#[cfg(not(target_arch = "wasm32"))]
mod lock;
mod nbt;
pub(crate) mod palette;
pub(crate) mod records;
#[cfg(not(target_arch = "wasm32"))]
pub mod session;
#[cfg(not(target_arch = "wasm32"))]
mod store;
#[cfg(not(target_arch = "wasm32"))]
mod terrain;
