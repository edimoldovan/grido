//! Letting Claude use Grido: the `grido` entry in the MCP clients' configs —
//! Claude Desktop's and Claude Code's — and the skill file that says how to
//! work with Grido's tools.
//!
//! The user decides when Grido first starts, and can change it in Settings.
//! The choice is kept in `claude.json` in Grido's config dir. Yes writes the
//! entry into every client config there is, and the skill; no — or switching
//! it off later — takes both out again. A client's config is merged, never
//! replaced, and the previous contents are left as a `.bak`.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// The name Grido's server goes by in a client's `mcpServers`.
const ENTRY: &str = "grido";

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// The config files of the MCP clients Post writes itself into.
fn clients() -> Vec<PathBuf> {
    let Some(home) = home() else {
        return Vec::new();
    };
    vec![
        home.join(".config/Claude/claude_desktop_config.json"),
        home.join(".claude.json"),
    ]
}

/// `$XDG_CONFIG_HOME/grido`, or `~/.config/grido`.
pub fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| home().map(|home| home.join(".config")))
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("grido")
}

fn choice_file() -> PathBuf {
    config_dir().join("claude.json")
}

/// What the user chose: `None` until they have been asked.
pub fn choice() -> Option<bool> {
    let text = std::fs::read_to_string(choice_file()).ok()?;
    serde_json::from_str::<Value>(&text)
        .ok()?
        .get("allow")?
        .as_bool()
}

/// Keeps the choice, and makes the clients and the skill match it.
pub fn choose(allow: bool) -> Result<(), String> {
    let file = choice_file();
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("could not make {}: {e}", dir.display()))?;
    }
    std::fs::write(&file, json!({ "allow": allow }).to_string())
        .map_err(|e| format!("could not write {}: {e}", file.display()))?;
    if allow { register() } else { unregister() }
}

/// Keeps a yes current: the entry pointing at this binary, the skill as this
/// version writes it. Nothing when the user said no or has not been asked.
pub fn refresh() {
    if choice() == Some(true) {
        let _ = register();
    }
}

/// Writes the entry into every client config there is, and the skill.
fn register() -> Result<(), String> {
    let entry = server_entry()?;
    for path in clients().iter().filter(|path| path.exists()) {
        register_one(path, &entry)?;
    }
    install_skill()
}

/// Takes the entry out of every client config, and the skill with it.
fn unregister() -> Result<(), String> {
    for path in clients().iter().filter(|path| path.exists()) {
        unregister_one(path)?;
    }
    remove_skill()
}

/// The command a client runs to reach this install.
fn server_entry() -> Result<Value, String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot find the grido binary: {e}"))?;
    Ok(json!({
        "command": exe.display().to_string(),
        "args": ["mcp"],
    }))
}

/// Adds or refreshes the entry in one client config. Only the keys
/// registration writes are compared, so a client that keeps its own beside
/// them is left as it is. False when it was already right.
pub fn register_one(path: &Path, entry: &Value) -> Result<bool, String> {
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let mut root = parse(path, &existing)?;
    if let Some(current) = root.pointer(&format!("/mcpServers/{ENTRY}"))
        && ["command", "args"].iter().all(|k| current.get(k) == entry.get(k))
    {
        return Ok(false);
    }
    let servers = root
        .as_object_mut()
        .expect("an object")
        .entry("mcpServers")
        .or_insert_with(|| json!({}));
    if !servers.is_object() {
        *servers = json!({});
    }
    servers
        .as_object_mut()
        .expect("an object")
        .insert(ENTRY.to_string(), entry.clone());
    write(path, &existing, &root)?;
    Ok(true)
}

/// Takes the entry out of one client config. False when it was not there.
pub fn unregister_one(path: &Path) -> Result<bool, String> {
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let mut root = parse(path, &existing)?;
    let removed = root
        .get_mut("mcpServers")
        .and_then(Value::as_object_mut)
        // shift_remove, not remove: the rest of the list keeps its order.
        .and_then(|servers| servers.shift_remove(ENTRY))
        .is_some();
    if removed {
        write(path, &existing, &root)?;
    }
    Ok(removed)
}

/// A client config as JSON: an empty or missing file is an empty object.
fn parse(path: &Path, text: &str) -> Result<Value, String> {
    if text.trim().is_empty() {
        return Ok(json!({}));
    }
    let root: Value =
        serde_json::from_str(text).map_err(|_| format!("{} is not valid JSON", path.display()))?;
    Ok(if root.is_object() { root } else { json!({}) })
}

/// Writes a client config back, leaving what it said before as `.bak`.
fn write(path: &Path, before: &str, root: &Value) -> Result<(), String> {
    if !before.trim().is_empty() {
        let _ = std::fs::write(path.with_extension("json.bak"), before);
    }
    let text = serde_json::to_string_pretty(root).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| format!("could not write {}: {e}", path.display()))
}

fn skill_dir() -> Option<PathBuf> {
    home().map(|home| home.join(".claude/skills").join(ENTRY))
}

/// Writes the skill if it is missing or out of date.
fn install_skill() -> Result<(), String> {
    let Some(dir) = skill_dir() else {
        return Ok(());
    };
    let path = dir.join("SKILL.md");
    let text = skill();
    if std::fs::read_to_string(&path).is_ok_and(|current| current == text) {
        return Ok(());
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("could not make {}: {e}", dir.display()))?;
    std::fs::write(&path, text).map_err(|e| format!("could not write {}: {e}", path.display()))
}

/// Takes the skill out, when it is the one Grido wrote.
fn remove_skill() -> Result<(), String> {
    let Some(dir) = skill_dir() else {
        return Ok(());
    };
    let ours = std::fs::read_to_string(dir.join("SKILL.md"))
        .is_ok_and(|text| text.starts_with("---\nname: grido\n"));
    if ours {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("could not remove {}: {e}", dir.display()))?;
    }
    Ok(())
}

/// How to work with Grido's tools, for the assistant: who Grido is, then
/// the guide.
pub fn skill() -> String {
    format!("{HEAD}\n{}", crate::mcp::skill::GUIDE)
}

const HEAD: &str = r#"---
name: grido
description: Work with the user's own spreadsheets in Grido through its MCP tools — build, update, query or tidy a .xlsx they keep: trackers, budgets, logs, contact lists. Use for "my spreadsheet", a .xlsx they maintain, or anything they keep in rows and columns.
---

# Grido

Grido is the user's spreadsheet on this computer, reached through the grido MCP
server. Every tool is named `grido_` and a verb. Grido is a window on the
user's screen; the tools and the window show the same workbook.
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("grido-claude-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch dir");
        dir
    }

    /// Registering adds the entry and keeps everything else; doing it again
    /// changes nothing; unregistering takes only the entry out.
    #[test]
    fn the_entry_goes_in_and_out_and_nothing_else_moves() {
        let path = scratch("register").join(".claude.json");
        std::fs::write(&path, r#"{"theme":"dark","mcpServers":{"other":{"command":"x"}}}"#)
            .expect("written");
        let entry = json!({"command": "/usr/bin/grido", "args": ["mcp"]});

        assert!(register_one(&path, &entry).expect("registered"));
        assert!(!register_one(&path, &entry).expect("again"), "already right");
        let root: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(root["mcpServers"]["grido"], entry);
        assert_eq!(root["mcpServers"]["other"]["command"], "x");
        assert_eq!(root["theme"], "dark");
        assert!(path.with_extension("json.bak").exists(), "the old one is kept");

        assert!(unregister_one(&path).expect("unregistered"));
        assert!(!unregister_one(&path).expect("again"), "already gone");
        let root: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(root["mcpServers"].get("grido").is_none());
        assert_eq!(root["mcpServers"]["other"]["command"], "x");
    }

    /// The skill is Grido's own, under its own name, with the guide in it.
    #[test]
    fn the_skill_is_gridos_own() {
        let skill = skill();
        assert!(skill.starts_with("---\nname: grido\n"));
        assert!(skill.contains("# Working with Grido") && skill.contains("grido_workbook_info"));
        assert!(!skill.to_lowercase().contains("armor"));
    }
}
