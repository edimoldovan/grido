//! Model Context Protocol: the socket the server listens on, the stdio shim
//! an MCP client launches, and the channel that carries a call from a socket
//! thread to the window's thread. The `grido_` tools themselves live in the
//! `grido` crate and are reached by plain function calls.

pub mod bridge;
pub mod proto;
