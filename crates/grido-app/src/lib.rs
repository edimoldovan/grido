//! Grido: a spreadsheet window, and the tools an assistant drives it with.
//!
//! The binary is a thin host around its roles, all the same executable:
//!
//! - `grido mcp` — the stdio shim an assistant launches. Windowless.
//! - `grido --serve` — the server: it owns the socket, knows which process
//!   shows the window, and holds the confirmation broker. One per user, and
//!   headless.
//! - `grido --confirm` — one confirmation dialog, started by the server for
//!   one question and gone once it is answered.
//! - `grido [file]` — the window. It claims the role, or hands its file to the
//!   window that has it.

pub mod app;
pub mod confirm;
pub mod dialog;
pub mod host;
pub mod ipc;
pub mod mcp;
pub mod role;
pub mod serve;
