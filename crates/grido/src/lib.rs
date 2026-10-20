//! Grido — a fast native spreadsheet.
//!
//! The `grido` binary (`grido-app`) is a thin host around this library, so
//! integration tests can drive the engine directly.
//!
//! The look and feel — theme, fonts, the elements, shared icons — comes from
//! omarchy-kit; what lives here is the spreadsheet itself.

pub mod app;
pub mod claude;
pub mod commands;
pub mod engine;
mod functions;
pub mod mcp;
pub mod keymap;
pub mod state;
pub mod ui;
