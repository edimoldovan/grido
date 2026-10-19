//! The dialogs, the AutoFilter's dialog, the sheet tabs and the status line.

use eframe::egui::{self, Color32, Context, Key, RichText, ScrollArea, TopBottomPanel, Ui};
use omarchy_kit::{Button, Checkbox, ColorButton, Dialog, Dropdown, NumberField, Slider, StatusLine, Swatches, TextField, Toggle};

use crate::commands::Command;
use crate::engine::{col_letters, parse_cell_ref, BorderTarget, HAlign, StyleEdit, VAlign};
use crate::state::{Dialog as Open, Pending, GridoApp};
use crate::ui::icons::glyph;
use crate::ui::top::{hex, rgb, unrgb};

/// Draft state for the AutoFilter dialog. Kept in egui's temp memory so it
/// lives only as long as the dialog is open.
#[derive(Clone, Default)]
struct FilterDraft {
    col: i32,
    search: String,
    /// (display value, checked)
    items: Vec<(String, bool)>,
}

/// Draft state for the Format cells dialog, reloaded from the active cell
/// whenever that cell changes.
#[derive(Clone)]
struct FormatDraft {
    /// (sheet, row, col) the controls were loaded from.
    key: (u32, i32, i32),
    /// 0 General, 1 Number, 2 Currency, 3 Percent, 4 Comma, 5 Date, 6 Custom
    num_kind: usize,
    decimals: i32,
    custom: String,
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    size: i32,
    font_color: Option<[u8; 3]>,
    fill: Option<[u8; 3]>,
    border_color: [u8; 3],
    align: HAlign,
    valign: VAlign,
    wrap: bool,
}

/// Extra sort levels; level 1 lives in `GridoApp::sort`.
#[derive(Clone, Copy)]
struct SortLevels {
    on2: bool,
    col2: i32,
    asc2: bool,
    on3: bool,
    col3: i32,
    asc3: bool,
}

impl Default for SortLevels {
    fn default() -> Self {
        SortLevels {
            on2: false,
            col2: 1,
            asc2: true,
            on3: false,
            col3: 1,
            asc3: true,
        }
    }
}

/// The number kinds the Format cells dialog offers, in its order.
const NUMBER_KINDS: [&str; 7] = ["General", "Number", "Currency", "Percent", "Comma", "Date", "Custom"];

/// Builds a number-format code from the dialog's controls.
fn num_fmt_code(kind: usize, decimals: i32, custom: &str) -> String {
    let with_decimals = |base: &str| {
        if decimals <= 0 {
            base.to_string()
        } else {
            format!("{base}.{}", "0".repeat(decimals as usize))
        }
    };
    match kind {
        1 => with_decimals("0"),
        2 => format!("\"$\"{}", with_decimals("#,##0")),
        3 => format!("{}%", with_decimals("0")),
        4 => with_decimals("#,##0"),
        5 => "yyyy-mm-dd".to_string(),
        6 => custom.to_string(),
        _ => "general".to_string(),
    }
}

/// Best-effort reverse of `num_fmt_code`, for showing the cell's current format.
pub(crate) fn detect_num_fmt(code: &str) -> (usize, i32) {
    let lower = code.to_lowercase();
    let decimals = lower
        .split_once('.')
        .map(|(_, rest)| rest.chars().take_while(|c| *c == '0').count() as i32)
        .unwrap_or(0);
    let kind = if lower.is_empty() || lower == "general" {
        0
    } else if lower.contains('%') {
        3
    } else if lower.contains('$') {
        2
    } else if lower.contains('y') || lower.contains("dd") || lower.contains("mmm") {
        5
    } else if lower.contains("#,##") {
        4
    } else if lower.chars().all(|c| "0#.,".contains(c)) {
        1
    } else {
        6
    };
    (kind, decimals)
}

/// A column chooser, by its A1 letters.
fn column_picker(ui: &mut Ui, id: &str, col: &mut i32, max_col: i32) {
    let letters: Vec<String> = (1..=max_col).map(col_letters).collect();
    let options: Vec<&str> = letters.iter().map(String::as_str).collect();
    let mut chosen = (*col - 1).max(0) as usize;
    if ui.add(Dropdown::new(&mut chosen, &options).width(88.0).id_salt(id)).changed() {
        *col = chosen as i32 + 1;
    }
}

/// A or Z first, as one control.
fn direction(ui: &mut Ui, ascending: &mut bool) {
    let mut chosen = usize::from(!*ascending);
    if ui.add(Toggle::new(&mut chosen, &["A → Z", "Z → A"])).changed() {
        *ascending = chosen == 0;
    }
}

/// A part of a dialog's caption, quietly above what it holds.
fn section(ui: &mut Ui, caption: &str) {
    ui.add_space(10.0);
    ui.label(RichText::new(caption).size(13.0).weak());
    ui.add_space(2.0);
}

/// True when Enter was what took the keyboard off `field`.
fn entered(ui: &Ui, field: &egui::Response) -> bool {
    field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter))
}

impl GridoApp {
    pub fn dialogs(&mut self, ctx: &Context) -> Vec<Command> {
        // The filter's dialog is not an `Open` dialog: it belongs to a column.
        self.filter_popup(ctx);
        match self.dialog.clone() {
            Open::None => {}
            Open::FindReplace => self.find_replace_dialog(ctx),
            Open::GotoCell => self.goto_dialog(ctx),
            Open::RenameSheet { sheet, name } => self.rename_dialog(ctx, sheet, name),
            Open::TabColor { sheet } => self.tab_color_dialog(ctx, sheet),
            Open::Sort => self.sort_dialog(ctx),
            Open::NameManager => self.names_dialog(ctx),
            Open::FormatCells => self.format_dialog(ctx),
            Open::ConditionalFormat => self.cf_dialog_impl(ctx),
            Open::Chart => self.chart_dialog_impl(ctx),
        }
        self.chart_windows(ctx);
        Vec::new()
    }

    // ----- AutoFilter -----

    fn filter_popup(&mut self, ctx: &Context) {
        let Some(filter) = self.filter.clone() else {
            return;
        };
        let Some(col) = filter.open_column else {
            return;
        };
        let id = egui::Id::new("filter-draft");
        let mut draft: FilterDraft = ctx.data_mut(|d| d.get_temp(id)).unwrap_or_default();
        if draft.col != col {
            let values =
                self.engine
                    .distinct_values(self.sheet, col, filter.header_row + 1, filter.r1);
            let allowed = filter.allowed.get(&col);
            draft = FilterDraft {
                col,
                search: String::new(),
                items: values
                    .into_iter()
                    .map(|v| {
                        // A missing or empty entry means "everything allowed".
                        let on = allowed.map(|a| a.is_empty() || a.contains(&v)).unwrap_or(true);
                        (v, on)
                    })
                    .collect(),
            };
        }

        let (mut apply, mut clear, mut cancel) = (false, false, false);
        let shown = Dialog::new(format!("Filter — column {}", col_letters(col)))
            .id_salt("grido-filter")
            .width(300.0)
            .show(ctx, |ui| {
                ui.add(TextField::new(&mut draft.search).hint("Search").id_salt("grido-filter-search"));
                ui.add_space(6.0);
                let needle = draft.search.to_lowercase();
                let visible: Vec<usize> = draft
                    .items
                    .iter()
                    .enumerate()
                    .filter(|(_, (v, _))| needle.is_empty() || v.to_lowercase().contains(&needle))
                    .map(|(i, _)| i)
                    .collect();
                let mut all = visible.iter().all(|i| draft.items[*i].1);
                if ui.add(Checkbox::new(&mut all, "Select all")).changed() {
                    for i in &visible {
                        draft.items[*i].1 = all;
                    }
                }
                ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                    for i in &visible {
                        let (value, checked) = &mut draft.items[*i];
                        let label = if value.is_empty() { "(Blanks)".to_string() } else { value.clone() };
                        ui.add(Checkbox::new(checked, label));
                    }
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    apply = ui.add(Button::new("OK").primary()).clicked();
                    cancel = ui.add(Button::new("Cancel")).clicked();
                });
                ui.add_space(6.0);
                clear = ui.add(Button::new("Clear this column's filter").quiet().small()).clicked();
            });

        if apply || clear || cancel || shown.closed {
            if let Some(f) = &mut self.filter {
                if apply {
                    let all = draft.items.iter().all(|(_, on)| *on);
                    let allowed: Vec<String> = if all {
                        Vec::new()
                    } else {
                        draft.items.iter().filter(|(_, on)| *on).map(|(v, _)| v.clone()).collect()
                    };
                    f.allowed.insert(col, allowed);
                } else if clear {
                    f.allowed.remove(&col);
                }
                f.open_column = None;
            }
            ctx.data_mut(|d| d.remove::<FilterDraft>(id));
            if apply || clear {
                self.apply_filter();
            }
        } else {
            ctx.data_mut(|d| d.insert_temp(id, draft));
        }
    }

    // ----- find -----

    fn find_replace_dialog(&mut self, ctx: &Context) {
        let shown = Dialog::new("Find and replace").modeless().width(400.0).show(ctx, |ui| {
            let find = ui.add(TextField::new(&mut self.find.needle).hint("Find").id_salt("grido-find"));
            if find.changed() {
                self.find.hits.clear();
            }
            if entered(ui, &find) {
                self.run_find();
            }
            ui.add_space(6.0);
            ui.add(TextField::new(&mut self.find.replacement).hint("Replace with").id_salt("grido-replace"));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add(Checkbox::new(&mut self.find.match_case, "Match case"));
                ui.add_space(12.0);
                ui.add(Checkbox::new(&mut self.find.whole_workbook, "Every sheet"));
            });
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.add(Button::new("Find all").primary()).clicked() {
                    self.run_find();
                }
                if ui.add(Button::new("Next")).clicked() {
                    self.find_step(true);
                }
                if ui.add(Button::new("Previous")).clicked() {
                    self.find_step(false);
                }
                if ui.add(Button::new("Replace all")).clicked() {
                    let (needle, replacement, case, workbook) = (
                        self.find.needle.clone(),
                        self.find.replacement.clone(),
                        self.find.match_case,
                        self.find.whole_workbook,
                    );
                    let sheet = self.sheet;
                    let n = self.engine.replace_all(sheet, &needle, &replacement, case, workbook);
                    self.find.message = format!("Replaced {n}");
                    self.find.hits.clear();
                }
            });
            if !self.find.message.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new(&self.find.message).weak());
            }
        });
        if shown.closed || (!ctx.wants_keyboard_input() && ctx.input(|i| i.key_pressed(Key::Escape))) {
            self.dialog = Open::None;
        }
    }

    fn goto_dialog(&mut self, ctx: &Context) {
        let mut go: Option<(i32, i32)> = None;
        let shown = Dialog::new("Go to").width(300.0).show(ctx, |ui| {
            let field = ui.add(TextField::new(&mut self.goto_text).hint("A cell, such as B12").id_salt("grido-goto"));
            field.request_focus();
            ui.add_space(10.0);
            let submit = entered(ui, &field) | ui.add(Button::new("Go").primary()).clicked();
            if submit {
                go = parse_cell_ref(&self.goto_text);
            }
        });
        if let Some((row, col)) = go {
            self.set_cursor(row, col);
            self.goto_text.clear();
            self.dialog = Open::None;
        } else if shown.closed {
            self.dialog = Open::None;
        }
    }

    fn rename_dialog(&mut self, ctx: &Context, sheet: u32, mut name: String) {
        let mut apply = false;
        let shown = Dialog::new("Rename sheet").width(300.0).show(ctx, |ui| {
            let field = ui.add(TextField::new(&mut name).id_salt("grido-rename"));
            field.request_focus();
            ui.add_space(10.0);
            apply = entered(ui, &field) | ui.add(Button::new("Rename").primary()).clicked();
        });
        if apply {
            let result = self.engine.rename_sheet(sheet, &name);
            self.report("Rename", result);
            self.dialog = Open::None;
        } else if shown.closed {
            self.dialog = Open::None;
        } else {
            self.dialog = Open::RenameSheet { sheet, name };
        }
    }

    /// The colour a sheet's tab is marked with, from the swatches; a pick
    /// is applied at once and closes it.
    fn tab_color_dialog(&mut self, ctx: &Context, sheet: u32) {
        let recent: Vec<Color32> = self.recent_colors.iter().map(|c| rgb(*c)).collect();
        let mut colour = self.engine.sheet_color(sheet).map(rgb);
        let name = self.engine.sheet_names().get(sheet as usize).cloned().unwrap_or_default();
        let mut picked = false;
        let shown = Dialog::new(format!("Tab colour — {name}")).id_salt("grido-tab-colour").width(240.0).show(ctx, |ui| {
            picked = ui
                .add(Swatches::new(&mut colour).clear("No colour").recent(&recent).id_salt("grido-tab-swatches"))
                .changed();
        });
        if picked {
            // An empty string clears the tab colour in the engine.
            let value = colour.map(|c| hex(unrgb(c))).unwrap_or_default();
            let result = self.engine.set_sheet_color(sheet, &value);
            self.report("Tab colour", result);
            if let Some(c) = colour {
                self.push_recent_color(unrgb(c));
            }
            self.dialog = Open::None;
        } else if shown.closed {
            self.dialog = Open::None;
        }
    }

    fn sort_dialog(&mut self, ctx: &Context) {
        let id = egui::Id::new("sort-levels");
        let mut levels: SortLevels = ctx.data_mut(|d| d.get_temp(id)).unwrap_or_default();
        let (_, used_cols) = self.engine.used_range(self.sheet);
        let max_col = used_cols.clamp(3, 200);
        let mut apply = false;
        let shown = Dialog::new("Sort").width(360.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Sort by");
                column_picker(ui, "sort-col-1", &mut self.sort.key_column, max_col);
                direction(ui, &mut self.sort.ascending);
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add(Checkbox::new(&mut levels.on2, "Then by"));
                if levels.on2 {
                    column_picker(ui, "sort-col-2", &mut levels.col2, max_col);
                    direction(ui, &mut levels.asc2);
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add(Checkbox::new(&mut levels.on3, "Then by"));
                if levels.on3 {
                    column_picker(ui, "sort-col-3", &mut levels.col3, max_col);
                    direction(ui, &mut levels.asc3);
                }
            });
            ui.add_space(8.0);
            ui.add(Checkbox::new(&mut self.sort.has_header, "My data has headers"));
            ui.add_space(10.0);
            apply = ui.add(Button::new("Sort").primary()).clicked();
        });
        ctx.data_mut(|d| d.insert_temp(id, levels));
        if apply {
            let (ur, uc) = self.engine.used_range(self.sheet);
            let r0 = if self.sort.has_header { 2 } else { 1 };
            let range = crate::engine::Range { r0, r1: ur, c0: 1, c1: uc };
            let mut keys = vec![(self.sort.key_column, self.sort.ascending)];
            if levels.on2 {
                keys.push((levels.col2, levels.asc2));
            }
            if levels.on3 {
                keys.push((levels.col3, levels.asc3));
            }
            // `sort_range` is a stable sort, so sorting the least significant
            // key first and the primary key last gives multi-key ordering.
            let sheet = self.sheet;
            for (key, asc) in keys.iter().rev() {
                let result = self.engine.sort_range(sheet, range, *key, *asc);
                self.report("Sort", result);
            }
            self.invalidate_geometry();
            self.dialog = Open::None;
        } else if shown.closed {
            self.dialog = Open::None;
        }
    }

    fn names_dialog(&mut self, ctx: &Context) {
        let shown = Dialog::new("Names").width(420.0).show(ctx, |ui| {
            let names = self.engine.defined_names();
            if names.is_empty() {
                ui.label(RichText::new("No names yet.").weak());
            }
            let mut delete: Option<String> = None;
            for (name, formula) in &names {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(name).strong());
                    ui.label(RichText::new(formula).weak());
                    let trash = Button::icon_only(glyph("trash")).quiet().small();
                    if ui.add(trash).on_hover_text("Delete this name").clicked() {
                        delete = Some(name.clone());
                    }
                });
            }
            if let Some(name) = delete {
                let result = self.engine.delete_defined_name(&name);
                self.report("Delete name", result);
            }
            section(ui, "New name");
            ui.add(TextField::new(&mut self.names.new_name).hint("Name").id_salt("grido-name"));
            ui.add_space(6.0);
            ui.add(TextField::new(&mut self.names.new_formula).hint("Refers to, such as =Sheet1!A1:A10").id_salt("grido-refers"));
            ui.add_space(10.0);
            if ui.add(Button::new("Add").primary()).clicked() {
                let (name, formula) = (self.names.new_name.clone(), self.names.new_formula.clone());
                match self.engine.add_defined_name(&name, &formula) {
                    Ok(()) => {
                        self.names.new_name.clear();
                        self.names.new_formula.clear();
                        self.names.message = format!("Added {name}");
                    }
                    Err(e) => self.names.message = format!("{e}"),
                }
            }
            if !self.names.message.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new(&self.names.message).weak());
            }
        });
        if shown.closed {
            self.dialog = Open::None;
        }
    }

    // ----- format cells -----

    /// The Format cells dialog: number, font, alignment, border and fill, one
    /// under the other. Every control applies to the selection as soon as it
    /// changes; only the custom format code waits for its button.
    fn format_dialog(&mut self, ctx: &Context) {
        let id = egui::Id::new("format-draft");
        let key = (self.sheet, self.cursor.0, self.cursor.1);
        let mut draft: Option<FormatDraft> = ctx.data_mut(|d| d.get_temp(id));
        if draft.as_ref().map(|d| d.key) != Some(key) {
            let fmt = self.engine.format(key.0, key.1, key.2);
            let (num_kind, decimals) = detect_num_fmt(&fmt.num_fmt);
            draft = Some(FormatDraft {
                key,
                num_kind,
                decimals,
                custom: fmt.num_fmt.clone(),
                bold: fmt.bold,
                italic: fmt.italic,
                underline: fmt.underline,
                strike: fmt.strike,
                size: fmt.font_size as i32,
                font_color: fmt.font_color,
                fill: fmt.fill_color,
                border_color: [74, 74, 74],
                align: fmt.align,
                valign: fmt.valign,
                wrap: fmt.wrap,
            });
        }
        let mut draft = draft.expect("draft is set above");

        let shown = Dialog::new("Format cells").width(440.0).show(ctx, |ui| {
            ScrollArea::vertical().max_height(ctx.content_rect().height() * 0.7).show(ui, |ui| {
                self.format_number(ui, &mut draft);
                self.format_font(ui, &mut draft);
                self.format_alignment(ui, &mut draft);
                self.format_border(ui, &mut draft);
                self.format_fill(ui, &mut draft);
            });
        });
        ctx.data_mut(|d| d.insert_temp(id, draft));
        if shown.closed {
            ctx.data_mut(|d| d.remove::<FormatDraft>(id));
            self.dialog = Open::None;
        }
    }

    fn format_number(&mut self, ui: &mut Ui, draft: &mut FormatDraft) {
        section(ui, "Number");
        let mut changed = false;
        ui.horizontal(|ui| {
            changed |= ui.add(Dropdown::new(&mut draft.num_kind, &NUMBER_KINDS).width(140.0).id_salt("grido-num-kind")).changed();
            if (1..=4).contains(&draft.num_kind) {
                ui.label("Decimals");
                let mut decimals = draft.decimals as f32;
                if ui.add(NumberField::new(&mut decimals, 0.0..=10.0).width(72.0).id_salt("grido-decimals")).changed() {
                    draft.decimals = decimals as i32;
                    changed = true;
                }
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add(TextField::new(&mut draft.custom).hint("Format code").width(240.0).id_salt("grido-num-code"));
            if ui.add(Button::new("Apply code")).clicked() {
                draft.num_kind = 6;
                changed = true;
            }
        });
        if changed {
            let code = num_fmt_code(draft.num_kind, draft.decimals, &draft.custom);
            draft.custom = code.clone();
            self.style_selection(StyleEdit::NumFmt(code));
        }
    }

    fn format_font(&mut self, ui: &mut Ui, draft: &mut FormatDraft) {
        section(ui, "Font");
        ui.horizontal_wrapped(|ui| {
            if ui.add(Checkbox::new(&mut draft.bold, "Bold")).changed() {
                self.style_selection(StyleEdit::Bold(draft.bold));
            }
            if ui.add(Checkbox::new(&mut draft.italic, "Italic")).changed() {
                self.style_selection(StyleEdit::Italic(draft.italic));
            }
            if ui.add(Checkbox::new(&mut draft.underline, "Underline")).changed() {
                self.style_selection(StyleEdit::Underline(draft.underline));
            }
            if ui.add(Checkbox::new(&mut draft.strike, "Strikethrough")).changed() {
                self.style_selection(StyleEdit::Strike(draft.strike));
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Size");
            let mut size = draft.size as f32;
            if ui.add(NumberField::new(&mut size, 6.0..=96.0).suffix("pt").width(80.0).id_salt("grido-font-size")).changed() {
                draft.size = size as i32;
                self.style_selection(StyleEdit::FontSize(draft.size));
            }
            ui.add_space(12.0);
            ui.label("Colour");
            let recent: Vec<Color32> = self.recent_colors.iter().map(|c| rgb(*c)).collect();
            let mut colour = draft.font_color.map(rgb);
            if ui.add(ColorButton::new(&mut colour, glyph("font-color")).clear("Automatic").recent(&recent)).changed() {
                draft.font_color = colour.map(unrgb);
                self.apply_color(false, draft.font_color);
            }
        });
    }

    fn format_alignment(&mut self, ui: &mut Ui, draft: &mut FormatDraft) {
        section(ui, "Alignment");
        const ACROSS: [HAlign; 4] = [HAlign::General, HAlign::Left, HAlign::Center, HAlign::Right];
        const DOWN: [VAlign; 3] = [VAlign::Top, VAlign::Center, VAlign::Bottom];
        ui.horizontal(|ui| {
            ui.label("Across");
            let mut chosen = ACROSS.iter().position(|a| *a == draft.align).unwrap_or(0);
            if ui.add(Toggle::new(&mut chosen, &["General", "Left", "Centre", "Right"])).changed() {
                draft.align = ACROSS[chosen];
                self.style_selection(StyleEdit::Align(draft.align));
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Down");
            let mut chosen = DOWN.iter().position(|v| *v == draft.valign).unwrap_or(2);
            if ui.add(Toggle::new(&mut chosen, &["Top", "Centre", "Bottom"])).changed() {
                draft.valign = DOWN[chosen];
                self.style_selection(StyleEdit::VAlign(draft.valign));
            }
        });
        ui.add_space(6.0);
        if ui.add(Checkbox::new(&mut draft.wrap, "Wrap text")).changed() {
            self.style_selection(StyleEdit::Wrap(draft.wrap));
        }
    }

    fn format_border(&mut self, ui: &mut Ui, draft: &mut FormatDraft) {
        section(ui, "Border");
        ui.horizontal(|ui| {
            ui.label("Colour");
            let mut colour = Some(rgb(draft.border_color));
            if ui.add(ColorButton::new(&mut colour, glyph("fill-color"))).changed()
                && let Some(c) = colour
            {
                draft.border_color = unrgb(c);
            }
        });
        ui.add_space(6.0);
        let mut target = None;
        ui.horizontal_wrapped(|ui| {
            for (label, value) in [
                ("All", BorderTarget::All),
                ("Outer", BorderTarget::Outer),
                ("Bottom", BorderTarget::Bottom),
                ("Top", BorderTarget::Top),
                ("Left", BorderTarget::Left),
                ("Right", BorderTarget::Right),
                ("None", BorderTarget::None),
            ] {
                if ui.add(Button::new(label).small()).clicked() {
                    target = Some(value);
                }
            }
        });
        if let Some(target) = target {
            let (sheet, range) = (self.sheet, self.selection());
            let color = hex(draft.border_color);
            let result = self.engine.set_border(sheet, range, target, &color);
            self.report("Border", result);
            self.invalidate_geometry();
        }
    }

    fn format_fill(&mut self, ui: &mut Ui, draft: &mut FormatDraft) {
        section(ui, "Fill");
        let recent: Vec<Color32> = self.recent_colors.iter().map(|c| rgb(*c)).collect();
        let mut colour = draft.fill.map(rgb);
        if ui.add(ColorButton::new(&mut colour, glyph("fill-color")).clear("No fill").recent(&recent)).changed() {
            draft.fill = colour.map(unrgb);
            self.apply_color(true, draft.fill);
        }
    }

    /// Asked before unsaved changes are left behind.
    pub fn confirm_dialog(&mut self, ctx: &Context) {
        let Some(action) = self.pending.clone() else {
            return;
        };
        let shown = Dialog::new("Unsaved changes").show(ctx, |ui| {
            ui.label("This workbook has unsaved changes.");
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.add(Button::new("Save").primary()).clicked() {
                    return Some("save");
                }
                if ui.add(Button::new("Don't save").danger()).clicked() {
                    return Some("discard");
                }
                if ui.add(Button::new("Cancel")).clicked() {
                    return Some("cancel");
                }
                None
            })
            .inner
        });
        let choice = match shown.inner.flatten() {
            Some(choice) => Some(choice),
            None if shown.closed => Some("cancel"),
            None => None,
        };
        match choice {
            Some("save") => {
                self.pending = None;
                if self.do_save() {
                    self.run_pending(action, ctx);
                }
            }
            Some("discard") => {
                self.pending = None;
                // Discarding means the workbook is no longer "dirty" for the
                // purposes of the action we are about to run.
                self.engine.dirty = false;
                self.run_pending(action, ctx);
            }
            Some(_) => self.pending = None,
            None => {}
        }
    }

    pub fn sheet_tabs(&mut self, ctx: &Context) {
        TopBottomPanel::bottom("tabs").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let names = self.engine.sheet_names();
                let mut switch = None;
                let mut rename: Option<(u32, String)> = None;
                let mut tab_color: Option<u32> = None;
                // (sheet to move, destination index)
                let mut move_to: Option<(u32, u32)> = None;
                for (i, name) in names.iter().enumerate() {
                    let idx = i as u32;
                    let colour = self.engine.sheet_color(idx).map(rgb);
                    let tab = Button::new(name.as_str()).quiet().small().selected(idx == self.sheet).bar(colour);
                    let resp = ui.add(tab);
                    if resp.clicked() {
                        switch = Some(idx);
                    }
                    if resp.double_clicked() {
                        rename = Some((idx, name.clone()));
                    }
                    resp.context_menu(|ui| {
                        let item = |ui: &mut Ui, label: &str| ui.add(Button::new(label).quiet().small()).clicked();
                        if item(ui, "Rename") {
                            rename = Some((idx, name.clone()));
                            ui.close();
                        }
                        if item(ui, "Tab colour…") {
                            tab_color = Some(idx);
                            ui.close();
                        }
                        if item(ui, "Duplicate") {
                            let _ = self.engine.duplicate_sheet(idx);
                            ui.close();
                        }
                        if item(ui, "Move left") {
                            if idx > 0 {
                                move_to = Some((idx, idx - 1));
                            }
                            ui.close();
                        }
                        if item(ui, "Move right") {
                            if (idx as usize) + 1 < names.len() {
                                move_to = Some((idx, idx + 1));
                            }
                            ui.close();
                        }
                        if item(ui, "Delete") {
                            if names.len() > 1 {
                                let _ = self.engine.delete_sheet(idx);
                                switch = Some(0);
                            }
                            ui.close();
                        }
                    });
                }
                let plus = Button::icon_only(glyph("plus")).quiet().small();
                if ui.add(plus).on_hover_text(self.tip("New sheet", Command::NewSheet)).clicked()
                    && self.engine.new_sheet().is_ok()
                {
                    switch = Some(names.len() as u32);
                }
                if let Some((from, to)) = move_to {
                    // The menu acts on the clicked tab, so follow it to its new
                    // position.
                    self.switch_sheet(from);
                    let result = self.engine.move_sheet(from, to);
                    self.report("Move sheet", result);
                    self.sheet = to;
                }
                if let Some(s) = switch {
                    self.sheet = self.sheet.min((self.engine.sheet_names().len() as u32).saturating_sub(1));
                    self.switch_sheet(s);
                }
                if let Some((sheet, name)) = rename {
                    self.dialog = Open::RenameSheet { sheet, name };
                }
                if let Some(sheet) = tab_color {
                    self.dialog = Open::TabColor { sheet };
                }
            });
            ui.add_space(4.0);
        });
    }

    /// The line at the bottom: the workbook and its sheet, what just
    /// happened, and at the far end the selection's numbers and the zoom.
    pub fn status_line(&mut self, ctx: &Context) {
        let book = self
            .engine
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Book1".to_string());
        let place = if self.settings {
            format!("{book} — settings")
        } else {
            let sheet = self.engine.sheet_names().get(self.sheet as usize).cloned().unwrap_or_default();
            format!("{book} — {sheet}")
        };
        let mut numbers: Vec<String> = Vec::new();
        let sel = self.selection();
        if !self.settings && (sel.rows() > 1 || sel.cols() > 1) {
            let (count, numeric, sum, avg) = self.engine.stats(self.sheet, sel);
            if numeric > 0 {
                numbers.push(format!("Sum {sum:.2}"));
                numbers.push(format!("Average {avg:.2}"));
            }
            numbers.push(format!("Count {count}"));
        }
        let (rows, cols) = self.engine.frozen(self.sheet);
        if !self.settings && (rows > 0 || cols > 0) {
            numbers.push(format!("Frozen {rows} × {cols}"));
        }
        let mut zoom = (self.zoom * 100.0).round();
        let mut zoomed = false;
        let settings = self.settings;
        StatusLine::new("grido-status")
            .place(place)
            .message(self.status.clone())
            .end(|ui| {
                if !settings {
                    let slider = Slider::new(&mut zoom, 40.0..=400.0).decimals(0).suffix("%").small().width(190.0).id_salt("grido-zoom");
                    zoomed = ui.add(slider).changed();
                }
                for text in numbers.iter().rev() {
                    ui.add_space(8.0);
                    ui.label(text);
                }
            })
            .show(ctx);
        if zoomed {
            self.zoom = zoom / 100.0;
            self.invalidate_geometry();
        }
    }
}

/// Kept for the app module's Pending import to stay meaningful in one place.
pub type PendingAction = Pending;
