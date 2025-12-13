//! # Suzuri
//!
//! A high-performance, cross-platform text rendering library for Rust.
//!
//! ## Overview
//!
//! `Suzuri` provides a flexible and efficient way to render text using various backends (CPU, WGPU, etc.).
//! The core of the library is the [`FontSystem`], which coordinates font loading, text layout, and rendering.
//!
//! ## Usage
//!
//! ```rust,no_run
//! use suzuri::{FontSystem, text::TextLayoutConfig};
//!
//! // 1. Create a FontSystem
//! let font_system = FontSystem::new();
//! font_system.load_system_fonts();
//!
//! // 2. Prepare text data
//! // (See examples for how to build TextData)
//!
//! // 3. Configure layout and renderers
//! // font_system.cpu_init(...);
//! // font_system.wgpu_init(...);
//!
//! // 4. Layout and Render
//! // let layout = font_system.layout_text(&data, &config);
//! // font_system.wgpu_render(&layout, ...);
//! ```
//!
//! ## Features
//!
//! *   **Flexible Backend**: Supports CPU-based rendering and GPU acceleration (via WGPU).
//! *   **Advanced Layout**: Handles text wrapping, alignment, and multi-font shaping.
//! *   **Font Management**: Easy loading of system fonts and custom font files.
//! *   **Thread Safety**: Designed with internal locking for safe concurrent use.

pub mod font_storage;
pub mod font_system;
pub mod glyph_id;
pub mod renderer;
pub mod text;

// common re-exports
pub use font_storage::FontStorage;
pub use font_system::FontSystem;
pub use glyph_id::GlyphId;

// re-export dependencies
pub use fontdb;
pub use fontdue;
pub use parking_lot;

#[cfg(feature = "wgpu")]
pub use wgpu;
