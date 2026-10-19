//! Native chart rendering and the conditional-formatting dialog.
//!
//! Charts are drawn with egui's painter — no plotting library, no xlsx chart
//! XML (charts are a view over a range and are not saved into the workbook).

use eframe::egui::{self, Align2, Color32, Context, RichText};
use omarchy_kit::{Button, ColorButton, Dialog, Dropdown, TextField, Toggle};

use crate::state::{Chart, ChartKind, Dialog as Open, GridoApp};
use crate::ui::icons::glyph;
use crate::ui::top::{rgb, unrgb};

/// What a cell-value rule compares with, in the order the dialog lists it.
const OPERATORS: [&str; 4] = ["greater than", "less than", "equal to", "between"];

impl GridoApp {
    pub fn cf_dialog_impl(&mut self, ctx: &Context) {
        let shown = Dialog::new("Conditional formatting").width(440.0).show(ctx, |ui| {
            ui.label(RichText::new("Rules apply to the cells selected.").weak());
            ui.add_space(8.0);
            ui.add(Toggle::new(&mut self.cf.kind, &["Cell value", "Colour scale"]));
            ui.add_space(8.0);
            if self.cf.kind == 0 {
                ui.horizontal(|ui| {
                    ui.add(Dropdown::new(&mut self.cf.operator, &OPERATORS).width(140.0).id_salt("grido-cf-op"));
                    ui.add(TextField::new(&mut self.cf.value).width(90.0).id_salt("grido-cf-value"));
                    if self.cf.operator == 3 {
                        ui.label("and");
                        ui.add(TextField::new(&mut self.cf.value2).width(90.0).id_salt("grido-cf-value2"));
                    }
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label("Fill");
                    let mut colour = Some(rgb(self.cf.fill));
                    if ui.add(ColorButton::new(&mut colour, glyph("fill-color")).tip("Fill colour")).changed()
                        && let Some(c) = colour
                    {
                        self.cf.fill = unrgb(c);
                    }
                });
            }
            ui.add_space(10.0);
            if ui.add(Button::new("Add rule").primary()).clicked() {
                let range = self.selection();
                let sheet = self.sheet;
                let result = if self.cf.kind == 1 {
                    self.engine.add_cf_color_scale(sheet, range)
                } else {
                    let op = ["greater", "less", "equal", "between"][self.cf.operator];
                    let hex = crate::ui::top::hex(self.cf.fill);
                    let second = if self.cf.operator == 3 { Some(self.cf.value2.as_str()) } else { None };
                    let value = self.cf.value.clone();
                    self.engine.add_cf_cell_is(sheet, range, op, &value, second, &hex)
                };
                match result {
                    Ok(()) => self.cf.message = "Rule added".to_string(),
                    Err(e) => self.cf.message = format!("{e}"),
                }
                self.invalidate_geometry();
            }
            if !self.cf.message.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new(&self.cf.message).weak());
            }

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(6.0);
            ui.label(RichText::new("Rules on this sheet").size(13.0).weak());
            let rules = self.engine.cf_rules(self.sheet);
            if rules.is_empty() {
                ui.label(RichText::new("None yet.").weak());
            }
            let mut delete: Option<u32> = None;
            for (index, range, kind) in &rules {
                ui.horizontal(|ui| {
                    ui.label(format!("{kind} — {range}"));
                    let trash = Button::icon_only(glyph("trash")).quiet().small().tip("Delete this rule");
                    if ui.add(trash).clicked() {
                        delete = Some(*index);
                    }
                });
            }
            if let Some(index) = delete {
                let sheet = self.sheet;
                let result = self.engine.delete_cf_rule(sheet, index);
                self.report("Delete rule", result);
                self.invalidate_geometry();
            }
        });
        if shown.closed {
            self.dialog = Open::None;
        }
    }

    pub fn chart_dialog_impl(&mut self, ctx: &Context) {
        let mut create = false;
        let shown = Dialog::new("Insert chart").width(340.0).show(ctx, |ui| {
            let sel = self.selection();
            ui.label(format!(
                "Data: {} ({}×{})",
                crate::ui::grid::range_name(sel),
                sel.rows(),
                sel.cols()
            ));
            ui.add_space(8.0);
            const KINDS: [ChartKind; 3] = [ChartKind::Bar, ChartKind::Line, ChartKind::Pie];
            let mut chosen = KINDS.iter().position(|k| *k == self.chart_draft).unwrap_or(0);
            if ui.add(Toggle::new(&mut chosen, &["Bar", "Line", "Pie"])).changed() {
                self.chart_draft = KINDS[chosen];
            }
            ui.add_space(10.0);
            if sel.rows() < 2 && sel.cols() < 2 {
                ui.label(RichText::new("Select a range with data first.").weak());
            } else if ui.add(Button::new("Create chart").primary()).clicked() {
                create = true;
            }
        });
        if create {
            let sel = self.selection();
            let has_header = (sel.c0..=sel.c1).any(|c| !self.engine.is_number(self.sheet, sel.r0, c));
            self.charts.push(Chart {
                kind: self.chart_draft,
                sheet: self.sheet,
                data: sel,
                title: format!("Chart {}", self.charts.len() + 1),
                has_header,
            });
            self.dialog = Open::None;
        } else if shown.closed {
            self.dialog = Open::None;
        }
    }

    /// A panel beside the sheet for each chart made on it.
    pub fn chart_windows(&mut self, ctx: &Context) {
        let charts: Vec<(usize, ChartKind, crate::engine::Range, String, u32, bool)> = self
            .charts
            .iter()
            .enumerate()
            .map(|(i, c)| (i, c.kind, c.data, c.title.clone(), c.sheet, c.has_header))
            .collect();
        let mut close: Option<usize> = None;
        for (index, kind, data, title, sheet, has_header) in charts {
            if sheet != self.sheet {
                continue;
            }
            let shown = Dialog::new(title)
                .id_salt(("grido-chart", index))
                .resizable(egui::Vec2::new(420.0, 280.0))
                .show(ctx, |ui| {
                    self.paint_chart(ui, kind, data, sheet, has_header);
                });
            if shown.closed {
                close = Some(index);
            }
        }
        if let Some(index) = close {
            self.charts.remove(index);
        }
    }

    fn paint_chart(
        &self,
        ui: &mut egui::Ui,
        kind: ChartKind,
        data: crate::engine::Range,
        sheet: u32,
        has_header: bool,
    ) {
        // Series: use the last column of numbers; labels from the first column.
        let first_data_row = if has_header { data.r0 + 1 } else { data.r0 };
        let label_col = data.c0;
        let value_col = if data.cols() > 1 { data.c1 } else { data.c0 };

        let mut points: Vec<(String, f64)> = Vec::new();
        for r in first_data_row..=data.r1 {
            let raw = self.engine.display(sheet, r, value_col);
            let Ok(value) = raw.replace(',', "").replace('$', "").trim().parse::<f64>() else {
                continue;
            };
            let label = if data.cols() > 1 {
                self.engine.display(sheet, r, label_col)
            } else {
                format!("{r}")
            };
            points.push((label, value));
        }

        if points.is_empty() {
            ui.label("No numeric data in the selected range.");
            return;
        }

        let palette = [
            Color32::from_rgb(74, 134, 232),
            Color32::from_rgb(224, 123, 57),
            Color32::from_rgb(106, 168, 79),
            Color32::from_rgb(204, 65, 37),
            Color32::from_rgb(142, 124, 195),
            Color32::from_rgb(212, 172, 54),
        ];
        let size = ui.available_size_before_wrap();
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        let painter = ui.painter().with_clip_rect(rect);
        let plot = rect.shrink2(egui::vec2(28.0, 22.0));
        let text_color = ui.visuals().text_color();
        let axis = egui::Stroke::new(1.0, ui.visuals().weak_text_color());
        let font = egui::FontId::proportional(10.0);

        match kind {
            ChartKind::Pie => {
                let total: f64 = points.iter().map(|(_, v)| v.abs()).sum();
                if total <= 0.0 {
                    ui.label("Values sum to zero.");
                    return;
                }
                let center = plot.center();
                let radius = plot.height().min(plot.width()) * 0.42;
                let mut start = -std::f32::consts::FRAC_PI_2;
                for (i, (label, value)) in points.iter().enumerate() {
                    let frac = (value.abs() / total) as f32;
                    let sweep = frac * std::f32::consts::TAU;
                    let color = palette[i % palette.len()];
                    // Approximate the wedge with a triangle fan.
                    let steps = (sweep / 0.12).ceil().max(2.0) as usize;
                    let mut mesh = egui::Mesh::default();
                    for s in 0..=steps {
                        let angle = start + sweep * (s as f32 / steps as f32);
                        let p = center + egui::vec2(angle.cos(), angle.sin()) * radius;
                        mesh.colored_vertex(p, color);
                    }
                    mesh.colored_vertex(center, color);
                    let hub = mesh.vertices.len() as u32 - 1;
                    for s in 0..steps as u32 {
                        mesh.add_triangle(hub, s, s + 1);
                    }
                    painter.add(egui::Shape::mesh(mesh));
                    let mid = start + sweep / 2.0;
                    let label_pos =
                        center + egui::vec2(mid.cos(), mid.sin()) * (radius + 14.0);
                    painter.text(
                        label_pos,
                        Align2::CENTER_CENTER,
                        format!("{label} {:.0}%", frac * 100.0),
                        font.clone(),
                        text_color,
                    );
                    start += sweep;
                }
            }
            ChartKind::Bar | ChartKind::Line => {
                let max = points.iter().map(|(_, v)| *v).fold(f64::MIN, f64::max);
                let min = points.iter().map(|(_, v)| *v).fold(f64::MAX, f64::min);
                let top = max.max(0.0);
                let bottom = min.min(0.0);
                let span = (top - bottom).max(1e-9);
                let y_of = |v: f64| -> f32 {
                    plot.max.y - ((v - bottom) / span) as f32 * plot.height()
                };
                // Axes.
                painter.line_segment(
                    [
                        egui::pos2(plot.min.x, y_of(0.0)),
                        egui::pos2(plot.max.x, y_of(0.0)),
                    ],
                    axis,
                );
                painter.line_segment([plot.left_top(), plot.left_bottom()], axis);
                painter.text(
                    plot.left_top() + egui::vec2(-4.0, 0.0),
                    Align2::RIGHT_TOP,
                    format!("{top:.0}"),
                    font.clone(),
                    text_color,
                );

                let n = points.len();
                let slot = plot.width() / n as f32;
                if kind == ChartKind::Bar {
                    for (i, (label, value)) in points.iter().enumerate() {
                        let x = plot.min.x + slot * i as f32;
                        let bar = egui::Rect::from_min_max(
                            egui::pos2(x + slot * 0.15, y_of(value.max(0.0))),
                            egui::pos2(x + slot * 0.85, y_of(value.min(0.0))),
                        );
                        painter.rect_filled(bar, 2.0, palette[i % palette.len()]);
                        painter.text(
                            egui::pos2(x + slot * 0.5, plot.max.y + 3.0),
                            Align2::CENTER_TOP,
                            elide(label, 8),
                            font.clone(),
                            text_color,
                        );
                    }
                } else {
                    let pts: Vec<egui::Pos2> = points
                        .iter()
                        .enumerate()
                        .map(|(i, (_, v))| {
                            egui::pos2(plot.min.x + slot * (i as f32 + 0.5), y_of(*v))
                        })
                        .collect();
                    painter.add(egui::Shape::line(
                        pts.clone(),
                        egui::Stroke::new(2.0, palette[0]),
                    ));
                    for (i, p) in pts.iter().enumerate() {
                        painter.circle_filled(*p, 3.0, palette[0]);
                        painter.text(
                            egui::pos2(p.x, plot.max.y + 3.0),
                            Align2::CENTER_TOP,
                            elide(&points[i].0, 8),
                            font.clone(),
                            text_color,
                        );
                    }
                }
            }
        }
    }
}

fn elide(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max - 1).collect::<String>())
    }
}
