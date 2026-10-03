//! Bedrock world persistence and interchange.
#[cfg(not(target_arch = "wasm32"))]
mod archive;
#[cfg(not(target_arch = "wasm32"))]
mod blocks;
#[cfg(not(target_arch = "wasm32"))]
pub mod cli;
#[cfg(not(target_arch = "wasm32"))]
mod lock;
#[cfg(not(target_arch = "wasm32"))]
mod nbt;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod palette;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod records;
#[cfg(not(target_arch = "wasm32"))]
pub mod session;
#[cfg(not(target_arch = "wasm32"))]
mod store;
#[cfg(not(target_arch = "wasm32"))]
mod terrain;
