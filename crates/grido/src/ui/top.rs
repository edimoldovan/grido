//! The bar along the top: Grido's mark, the tools a sheet is mostly worked
//! with, and at the far end the palette's button and the gear.

use eframe::egui::{
    Align, Align2, Color32, Context, FontId, Layout, Painter, Pos2, Rect, Sense, Shape, Stroke,
    TopBottomPanel, Ui, Vec2,
};
use omarchy_kit::{Button, ColorButton, Dropdown};

use crate::commands::Command;
use crate::engine::{HAlign, StyleEdit};
use crate::state::GridoApp;
use crate::ui::icons::glyph;

/// The number formats the bar's picker offers, in the order it lists them.
const NUMBER_FORMATS: [&str; 6] = ["General", "Number", "Currency", "Percent", "Comma", "Date"];

impl GridoApp {
    pub fn top_bar(&mut self, ctx: &Context) -> Vec<Command> {
        let mut cmds = Vec::new();
        TopBottomPanel::top("grido-top").show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                mark(ui);
                ui.add_space(14.0);
                if !self.settings {
                    self.tools(ui, &mut cmds);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let gear = Button::icon_only(glyph("gear"))
                        .quiet()
                        .selected(self.settings)
                        .tip("Settings")
                        .hint(self.key_of(Command::Settings).unwrap_or_default());
                    if ui.add(gear).clicked() {
                        cmds.push(Command::Settings);
                    }
                    let keys = Button::icon_only(glyph("keyboard"))
                        .quiet()
                        .selected(self.command_palette.is_open())
                        .tip("Actions and keys")
                        .hint(self.key_of(Command::Palette).unwrap_or_default());
                    if ui.add(keys).clicked() {
                        cmds.push(Command::Palette);
                    }
                });
            });
            ui.add_space(8.0);
        });
        cmds
    }

    /// The tools, in groups: what was done, the clipboard, the type, the
    /// alignment, the fill, the number format, the order, the filter, find.
    fn tools(&mut self, ui: &mut Ui, cmds: &mut Vec<Command>) {
        let can_undo = self.engine.can_undo();
        let can_redo = self.engine.can_redo();
        self.tool(ui, "undo", "Undo", Command::Undo, can_undo, cmds);
        self.tool(ui, "redo", "Redo", Command::Redo, can_redo, cmds);
        gap(ui);
        self.tool(ui, "cut", "Cut", Command::Cut, true, cmds);
        self.tool(ui, "copy", "Copy", Command::Copy, true, cmds);
        self.tool(ui, "paste", "Paste", Command::Paste, true, cmds);
        gap(ui);
        self.tool(ui, "bold", "Bold", Command::ToggleBold, true, cmds);
        self.tool(ui, "italic", "Italic", Command::ToggleItalic, true, cmds);
        gap(ui);
        self.tool(ui, "align-left", "Align left", Command::AlignLeft, true, cmds);
        self.tool(ui, "align-center", "Centre", Command::AlignCenter, true, cmds);
        self.tool(ui, "align-right", "Align right", Command::AlignRight, true, cmds);
        gap(ui);
        let fmt = self.engine.format(self.sheet, self.cursor.0, self.cursor.1);
        let mut fill = fmt.fill_color.map(rgb);
        let recent: Vec<Color32> = self.recent_colors.iter().map(|c| rgb(*c)).collect();
        let picked = ui
            .add(ColorButton::new(&mut fill, glyph("fill-color")).clear("No fill").recent(&recent).tip("Fill colour"))
            .changed();
        if picked {
            self.apply_color(true, fill.map(unrgb));
        }
        gap(ui);
        let (kind, _) = crate::ui::dialogs::detect_num_fmt(&fmt.num_fmt);
        let mut chosen = kind;
        let picker = Dropdown::new(&mut chosen, &NUMBER_FORMATS).placeholder("Custom").width(124.0);
        if ui.add(picker).on_hover_text("Number format").changed() {
            cmds.push(match chosen {
                1 => Command::FormatNumber,
                2 => Command::FormatCurrency,
                3 => Command::FormatPercent,
                4 => Command::FormatComma,
                5 => Command::FormatDate,
                _ => Command::FormatGeneral,
            });
        }
        gap(ui);
        self.tool(ui, "sort-asc", "Sort A to Z", Command::SortAscending, true, cmds);
        self.tool(ui, "sort-desc", "Sort Z to A", Command::SortDescending, true, cmds);
        self.tool(ui, "filter", "Filter", Command::ToggleFilter, true, cmds);
        gap(ui);
        self.tool(ui, "find", "Find and replace", Command::FindReplace, true, cmds);
    }

    /// One tool: a quiet icon, marked while what it sets is set, its name and
    /// key in the tooltip.
    fn tool(&self, ui: &mut Ui, icon: &'static str, name: &str, cmd: Command, enabled: bool, cmds: &mut Vec<Command>) {
        let button = Button::icon_only(glyph(icon))
            .quiet()
            .selected(self.command_is_active(cmd))
            .tip(name)
            .hint(self.key_of(cmd).unwrap_or_default());
        if ui.add_enabled(enabled, button).clicked() {
            cmds.push(cmd);
        }
    }

    /// A tooltip: what it does, and the key it is also on.
    pub fn tip(&self, name: &str, cmd: Command) -> String {
        match self.key_of(cmd) {
            Some(key) => format!("{name}  ({key})"),
            None => name.to_string(),
        }
    }

    /// The key `cmd` is on, as the window shows it. Of several, a letter's
    /// chord reads best, and a function key or a shifted one worst.
    pub fn key_of(&self, cmd: Command) -> Option<String> {
        let worse = |shortcut: &eframe::egui::KeyboardShortcut| {
            let name = shortcut.logical_key.name();
            let function = name.len() > 1 && name.starts_with('F') && name[1..].chars().all(|c| c.is_ascii_digit());
            let letter = name.len() == 1 && name.chars().all(|c| c.is_ascii_alphabetic());
            (function || shortcut.modifiers.shift, !letter)
        };
        let names = eframe::egui::ModifierNames::NAMES;
        self.keymap
            .bindings
            .iter()
            .filter(|(_, bound)| *bound == cmd)
            .map(|(shortcut, _)| shortcut)
            .min_by_key(|shortcut| worse(shortcut))
            .map(|shortcut| {
                // egui calls the slash and the comma Slash and Comma.
                let text = shortcut.format(&names, false);
                match shortcut.logical_key {
                    eframe::egui::Key::Slash => text.replace("Slash", "/"),
                    eframe::egui::Key::Comma => text.replace("Comma", ","),
                    _ => text,
                }
            })
    }

    /// Tools light up when the active cell (or sheet) is already in that
    /// state.
    pub fn command_is_active(&self, cmd: Command) -> bool {
        let fmt = self.engine.format(self.sheet, self.cursor.0, self.cursor.1);
        match cmd {
            Command::ToggleBold => fmt.bold,
            Command::ToggleItalic => fmt.italic,
            Command::AlignLeft => fmt.align == HAlign::Left,
            Command::AlignCenter => fmt.align == HAlign::Center,
            Command::AlignRight => fmt.align == HAlign::Right,
            Command::ToggleFilter => self.filter.is_some(),
            _ => false,
        }
    }

    /// Most recently used colours stay at the front, newest first, capped at
    /// ten.
    pub fn push_recent_color(&mut self, c: [u8; 3]) {
        self.recent_colors.retain(|x| *x != c);
        self.recent_colors.insert(0, c);
        self.recent_colors.truncate(10);
    }

    /// Applies a colour to the selection as its fill or its font colour.
    pub fn apply_color(&mut self, fill: bool, choice: Option<[u8; 3]>) {
        let value = choice.map(hex);
        let edit = if fill {
            StyleEdit::FillColor(value)
        } else {
            StyleEdit::FontColor(value)
        };
        let (sheet, sel) = (self.sheet, self.selection());
        let result = self.engine.set_style(sheet, sel, edit);
        self.report("Colour", result);
        if let Some(c) = choice {
            self.push_recent_color(c);
        }
        self.invalidate_geometry();
    }
}

/// Room between two groups of tools.
fn gap(ui: &mut Ui) {
    ui.add_space(10.0);
}

pub fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

pub fn unrgb(c: Color32) -> [u8; 3] {
    [c.r(), c.g(), c.b()]
}

pub fn hex(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

/// Grido's logo and its name, at the start of the bar.
fn mark(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(100.0, 28.0), Sense::hover());
    let square = Rect::from_min_size(Pos2::new(rect.min.x + 2.0, rect.center().y - 14.0), Vec2::splat(28.0));
    logo(ui.painter(), square);
    ui.painter().text(
        Pos2::new(square.max.x + 12.0, rect.center().y),
        Align2::LEFT_CENTER,
        "Grido",
        FontId::proportional(18.0),
        ui.visuals().strong_text_color(),
    );
}

/// Grido's logo, as `packaging/grido.svg` draws it on its 64-point square: a
/// white sheet on Grido's blue, its header row tinted and its grid lines in
/// a paler blue. Its own colours, whatever the theme.
pub fn logo(painter: &Painter, rect: Rect) {
    let scale = rect.width() / 64.0;
    let at = |x: f32, y: f32| rect.min + Vec2::new(x, y) * scale;
    let line = Color32::from_rgb(0xa9, 0xc4, 0xe6);
    painter.rect_filled(rect, 12.0 * scale, Color32::from_rgb(0x1d, 0x4f, 0x91));
    painter.rect_filled(Rect::from_min_max(at(10.0, 13.0), at(54.0, 51.0)), 2.0 * scale, Color32::WHITE);
    painter.rect_filled(
        Rect::from_min_max(at(10.0, 13.0), at(54.0, 22.0)),
        2.0 * scale,
        Color32::from_rgb(0xdb, 0xe7, 0xf5),
    );
    let stroke = Stroke::new(1.5 * scale, line);
    for y in [22.0, 31.5, 41.0] {
        painter.add(Shape::line_segment([at(10.0, y), at(54.0, y)], stroke));
    }
    for x in [24.0, 39.0] {
        painter.add(Shape::line_segment([at(x, 13.0), at(x, 51.0)], stroke));
    }
}
