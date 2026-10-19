//! The command palette: every action there is where the window is, found by
//! typing — the commands, each with its key, then the sheets to go to and the
//! workbooks opened last.

use std::path::PathBuf;

use eframe::egui::Context;
use omarchy_kit::Choice;

use crate::commands::Command;
use crate::state::{GridoApp, Pending};

/// What one line of the palette does.
#[derive(Clone, Debug, PartialEq)]
enum Action {
    Run(Command),
    Sheet(u32),
    Open(PathBuf),
}

impl GridoApp {
    /// The palette while it is open, and what is chosen in it done.
    pub fn palette_view(&mut self, ctx: &Context) {
        if !self.command_palette.is_open() {
            return;
        }
        let (choices, actions): (Vec<Choice>, Vec<Action>) = self.palette_lines().into_iter().unzip();
        let key = self.key_of(Command::Palette).unwrap_or_default();
        if let Some(action) = self.command_palette.show(ctx, &key, &choices).and_then(|i| actions.into_iter().nth(i)) {
            match action {
                Action::Run(cmd) => self.exec(cmd, ctx),
                Action::Sheet(sheet) => {
                    self.settings = false;
                    self.switch_sheet(sheet);
                }
                Action::Open(path) => {
                    self.settings = false;
                    if self.engine.dirty {
                        self.pending = Some(Pending::OpenPath(path));
                    } else {
                        self.open_path(path);
                    }
                }
            }
        }
    }

    /// Every line the palette offers: the commands that do something where
    /// the window is, then the other sheets, then the workbooks opened last.
    fn palette_lines(&self) -> Vec<(Choice, Action)> {
        let mut lines: Vec<(Choice, Action)> = Command::ALL
            .into_iter()
            .filter(|&cmd| self.applies(cmd))
            .map(|cmd| (Choice { label: cmd.label().to_string(), key: self.key_of(cmd) }, Action::Run(cmd)))
            .collect();
        let line = |label: String, action: Action| (Choice { label, key: None }, action);
        for (i, name) in self.engine.sheet_names().iter().enumerate() {
            if i as u32 != self.sheet || self.settings {
                lines.push(line(format!("Go to sheet {name}"), Action::Sheet(i as u32)));
            }
        }
        for path in &self.recent {
            if self.engine.path.as_ref() != Some(path) {
                let name = path.file_name().map_or(path.display().to_string(), |n| n.to_string_lossy().into_owned());
                lines.push(line(format!("Open {name}"), Action::Open(path.clone())));
            }
        }
        lines
    }

    /// Whether `cmd` does something where the window is: on Settings, only
    /// the workbook's own commands and the way back.
    fn applies(&self, cmd: Command) -> bool {
        match cmd {
            Command::Palette => false,
            Command::Undo => self.engine.can_undo(),
            Command::Redo => self.engine.can_redo(),
            _ if self.settings => matches!(
                cmd,
                Command::New | Command::Open | Command::Save | Command::SaveAs | Command::Settings | Command::Quit
            ),
            _ => true,
        }
    }
}
