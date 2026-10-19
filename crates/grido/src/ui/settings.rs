//! Settings: in place of the sheet, from the gear. Whether Claude may use
//! Grido, and what it did; and the question Grido asks about it when it
//! first starts.

use eframe::egui::{CentralPanel, Context, Key, RichText, ScrollArea, Ui};
use omarchy_kit::{Button, Dialog, Toggle};

use crate::state::GridoApp;
use crate::ui::icons::glyph;

/// What saying yes means, in the question and under the switch.
const WHAT: &str = "Claude can read your workbook and change it with Grido's tools. \
                    Changes ask you first, unless you allowed a tool for good.";

/// How wide the settings column is.
const COLUMN: f32 = 420.0;

impl GridoApp {
    pub fn settings_view(&mut self, ctx: &Context) {
        CentralPanel::default().show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let back = Button::icon_only(glyph("back")).quiet();
                if ui.add(back).on_hover_text("Back to the sheet  (Escape)").clicked() {
                    self.settings = false;
                }
                ui.label(RichText::new("Settings").size(17.0).strong());
            });
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(6.0);
            ui.allocate_ui(eframe::egui::Vec2::new(COLUMN.min(ui.available_width()), ui.available_height()), |ui| {
                ui.vertical(|ui| {
                    self.claude_setting(ui);
                    if !self.activity.is_empty() {
                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(6.0);
                        ui.label(RichText::new("What Claude did").size(14.0));
                        ui.add_space(4.0);
                        ScrollArea::vertical().show(ui, |ui| {
                            for entry in &self.activity {
                                let colour = if entry.ok {
                                    ui.visuals().weak_text_color()
                                } else {
                                    ui.visuals().error_fg_color
                                };
                                ui.label(RichText::new(&entry.text).size(12.5).color(colour));
                            }
                        });
                    }
                });
            });
        });
        if !ctx.wants_keyboard_input() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.settings = false;
        }
    }

    /// The switch, and what it means under it.
    fn claude_setting(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Claude").size(14.0));
            let mut on = usize::from(self.claude == Some(true));
            if ui.add(Toggle::new(&mut on, &["Off", "On"])).changed() {
                self.set_claude(on == 1);
            }
        });
        ui.add_space(4.0);
        ui.label(RichText::new(WHAT).weak().size(12.5));
    }

    /// The question, until it is answered: Grido asks it when it first starts.
    pub fn claude_question(&mut self, ctx: &Context) {
        if self.claude.is_some() {
            return;
        }
        let shown = Dialog::new("Let Claude use Grido?").width(420.0).show(ctx, |ui| {
            ui.label(WHAT);
            ui.add_space(6.0);
            ui.label(
                RichText::new(
                    "Grido adds itself to Claude Desktop and Claude Code. You can change \
                     this in Settings.",
                )
                .weak(),
            );
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                if ui.add(Button::new("Allow").primary()).clicked() {
                    return Some(true);
                }
                if ui.add(Button::new("Don't allow")).clicked() {
                    return Some(false);
                }
                None
            })
            .inner
        });
        if let Some(allow) = shown.inner.flatten() {
            self.set_claude(allow);
        }
    }

    /// Keeps the choice, and makes Claude's configs match it.
    fn set_claude(&mut self, allow: bool) {
        match crate::claude::choose(allow) {
            Ok(()) => self.claude = Some(allow),
            Err(e) => self.set_status(format!("Claude: {e}")),
        }
    }
}
