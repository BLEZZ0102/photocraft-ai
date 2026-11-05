//! Replaceable image editing providers. Service selection is explicit; never fall back.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::path::{Path, PathBuf};

#[cfg(not(target_arch = "wasm32"))]
pub mod api;
pub mod api_config;
pub mod api_error;
#[cfg(not(target_arch = "wasm32"))]
pub mod api_settings;
#[cfg(not(target_arch = "wasm32"))]
pub mod codex;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("AI edit cancelled")]
    Cancelled,
    #[error("AI edit timed out")]
    Timeout,
    #[error("{0}")]
    RuntimeUnavailable(String),
    #[error("{0}")]
    Provider(String),
    #[error("image service I/O: {0}")]
    Io(#[from] std::io::Error),
}

pub struct EditRequest<'a> {
    /// Flattened, colour-managed PNG of the current canvas.
    pub image: &'a Path,
    pub prompt: &'a str,
    pub workspace: &'a Path,
}

pub struct EditResult {
    pub saved_path: PathBuf,
}

/// A rendered layer reference; paths point only into the current private job workspace.
pub struct LayerReference {
    pub label: String,
    pub image: PathBuf,
}

/// Explicitly selected document context, separate from the user's instructions.
pub struct LayerContext {
    pub manifest: String,
    pub images: Vec<LayerReference>,
}

/// PhotoCraft's own bounded reference budget, not a service/model limit.
pub const MAX_LAYER_IMAGES: usize = 15;
pub const MAX_LAYER_MANIFEST_BYTES: usize = 64 * 1024;

impl LayerContext {
    /// Reject oversized context before opening a service connection.
    pub fn validate(&self) -> Result<(), Error> {
        if self.images.len() > MAX_LAYER_IMAGES || self.manifest.len() > MAX_LAYER_MANIFEST_BYTES || self.images.iter().any(|image| image.label.len() > 128) {
            return Err(Error::Provider("Layer context exceeds PhotoCraft's reference budget".into()));
        }
        Ok(())
    }
}

/// Providers own transport/authentication; the engine owns import, colour conversion and undo.
pub trait ImageProvider: Send + Sync {
    fn edit(&self, request: &EditRequest<'_>, cancelled: &dyn Fn() -> bool) -> Result<EditResult, Error>;

    fn edit_with_layers(&self, _request: &EditRequest<'_>, _layers: &LayerContext, _cancelled: &dyn Fn() -> bool) -> Result<EditResult, Error> {
        Err(Error::Provider("This provider does not support individual layer references".into()))
    }
}
