//! `grido --serve` as a real process, on a computer with no screen.
//!
//! The server has no window, so it has to come up and serve with `DISPLAY` and
//! `WAYLAND_DISPLAY` both unset — that is what these tests take away. What it
//! cannot do without is a way to ask the user, and that is a process of its own:
//! a stub script stands in for the `grido --confirm` window here, answering
//! down the same pipe a real dialog would.

#![cfg(target_os = "linux")]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use serde_json::{json, Value};

const EXE: &str = env!("CARGO_BIN_EXE_grido");

/// A dialog that refuses.
const DENIES: &str = r#"read question
printf '%s\n' '{"decision":"deny","always_allow":false}'"#;

/// A scratch home for one test. The socket, the allowlist and everything the
/// server writes on startup land under here, nowhere near the real ones.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("grido-tests")
        .join(format!("headless-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch dir");
    dir
}

/// Writes the stub dialog this test wants the server to run.
fn dialog(dir: &Path, body: &str) {
    let path = dir.join("dialog.sh");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write the stub dialog");
    std::fs::set_permissions(&path, PermissionsExt::from_mode(0o755)).expect("make it runnable");
}

/// A running server, killed when the test ends however it ends.
struct Serving {
    child: Child,
    dir: PathBuf,
    socket: PathBuf,
}

impl Drop for Serving {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Serving {
    /// True while the process is still up.
    fn running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    fn talk(&self) -> Talk {
        Talk::to(&self.socket)
    }
}

/// Starts `grido --serve` with no display of any kind, and waits for its
/// socket. The stub dialog from [`dialog`] is what it will ask with.
fn serve(dir: PathBuf) -> Serving {
    let socket = dir.join("grido.sock");
    let child = Command::new(EXE)
        .arg("--serve")
        // No screen, at all: if anything in this role reached for a window it
        // would die here instead of serving.
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        // The socket, the daemon it would ask, the allowlist and the assistant
        // configs it registers with: all under the scratch dir.
        .env("XDG_RUNTIME_DIR", &dir)
        .env("XDG_CONFIG_HOME", &dir)
        .env("XDG_DATA_HOME", &dir)
        .env("HOME", &dir)
        .env("GRIDO_CONFIRM_CMD", dir.join("dialog.sh"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("grido --serve should start");
    grido_app::ipc::wait_for_socket(&socket, Duration::from_secs(20))
        .expect("the headless server should bind its socket");
    Serving { child, dir, socket }
}

/// One MCP client on the server's socket.
struct Talk {
    out: UnixStream,
    reader: BufReader<UnixStream>,
}

impl Talk {
    fn to(socket: &Path) -> Talk {
        let out = UnixStream::connect(socket).expect("a connection to the server");
        out.set_read_timeout(Some(Duration::from_secs(30)))
            .expect("a read timeout, so a hang fails rather than waits");
        let reader = BufReader::new(out.try_clone().expect("clone"));
        Talk { out, reader }
    }

    fn send(&mut self, id: Value, method: &str, params: Value) {
        let line = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        writeln!(self.out, "{line}").expect("write");
        self.out.flush().expect("flush");
    }

    fn reply(&mut self) -> Value {
        let mut line = String::new();
        self.reader.read_line(&mut line).expect("a reply line");
        serde_json::from_str(&line).expect("json")
    }
}

/// The handshake and the tool list, with no display anywhere.
#[test]
fn the_server_serves_with_no_display() {
    let dir = scratch("no-display");
    dialog(&dir, DENIES);
    let mut serving = serve(dir);
    let mut talk = serving.talk();

    talk.send(json!("init"), "initialize", json!({}));
    let reply = talk.reply();
    assert_eq!(
        reply["result"]["protocolVersion"],
        grido_app::mcp::proto::PROTOCOL_VERSION
    );

    talk.send(json!(2), "tools/list", json!({}));
    let reply = talk.reply();
    assert_eq!(
        reply["result"]["tools"].as_array().expect("tools").len(),
        20,
        "the whole surface, served without a screen"
    );

    assert!(
        serving.running(),
        "a headless server does not exit for want of a display"
    );
}
