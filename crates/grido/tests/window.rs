//! The window, drawn with no screen: its bars keep their height however
//! often it is repainted.

use eframe::egui::{self, containers::panel::PanelState, Context, Event, Id, Pos2, RawInput, Rect, Vec2};
use grido::state::GridoApp;

/// The pointer moving over the sheet repaints it; the bars above and below
/// must come out the same height every time, or they push the sheet out of
/// the window.
#[test]
fn the_bars_keep_their_height() {
    let ctx = Context::default();
    let mut app = GridoApp::new(None);
    let mut heights = Vec::new();
    for i in 0..20 {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
            events: vec![Event::PointerMoved(Pos2::new(300.0 + i as f32 * 7.0, 320.0))],
            ..RawInput::default()
        };
        let _ = ctx.run(input, |ctx| {
            let _ = app.top_bar(ctx);
            app.status_line(ctx);
            app.formula_bar(ctx);
            app.sheet_tabs(ctx);
            let _ = app.grid(ctx);
        });
        let height = |id: &str| PanelState::load(&ctx, Id::new(id)).map_or(0.0, |s| s.rect.height());
        heights.push((height("formulabar"), height("grido-top"), height("tabs")));
    }
    let settled = &heights[2..];
    assert!(settled.windows(2).all(|w| w[0] == w[1]), "the bars changed height: {heights:?}");
    assert!(settled[0].0 < 80.0, "the formula bar is one line tall: {heights:?}");
}

/// One frame of the whole window, with these events.
fn frame(ctx: &Context, app: &mut GridoApp, events: Vec<Event>) {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
        events,
        ..RawInput::default()
    };
    let _ = ctx.run(input, |ctx| app.frame(ctx));
}

fn click(at: Pos2) -> Vec<Event> {
    let press = |pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    vec![Event::PointerMoved(at), press(true), press(false)]
}

/// The whole window, used the way a person uses it: the pointer over every
/// part of it, clicks on the bar and the sheet, typing into cells.
#[test]
fn the_window_survives_being_used() {
    let ctx = Context::default();
    let mut app = GridoApp::new(None);
    app.claude = Some(false);
    for y in (0..800).step_by(40) {
        for x in (0..1200).step_by(60) {
            frame(&ctx, &mut app, vec![Event::PointerMoved(Pos2::new(x as f32, y as f32))]);
        }
    }
    for x in (100..1150).step_by(48) {
        frame(&ctx, &mut app, click(Pos2::new(x as f32, 28.0)));
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
    }
    for (x, y) in [(200.0, 200.0), (400.0, 300.0), (600.0, 500.0)] {
        frame(&ctx, &mut app, click(Pos2::new(x, y)));
        frame(&ctx, &mut app, vec![Event::Text("12".into())]);
        frame(&ctx, &mut app, vec![Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
    }
    frame(&ctx, &mut app, vec![]);
}

/// Every dialog, the palette, Settings and the first question, each opened
/// and drawn with the pointer moving over it.
#[test]
fn every_screen_and_dialog_draws() {
    use grido::commands::Command;
    let ctx = Context::default();
    let mut app = GridoApp::new(None);
    app.claude = None;
    for i in 0..5 {
        frame(&ctx, &mut app, vec![Event::PointerMoved(Pos2::new(500.0 + i as f32, 300.0))]);
    }
    app.claude = Some(false);
    for cmd in [
        Command::Palette,
        Command::Palette,
        Command::Settings,
        Command::Settings,
        Command::FormatDialog,
        Command::FindReplace,
        Command::GotoCell,
        Command::NameManager,
        Command::ConditionalFormat,
        Command::InsertChart,
        Command::RenameSheet,
        Command::TabColor,
        Command::ToggleFilter,
        Command::New,
    ] {
        app.exec(cmd, &ctx);
        for i in 0..6 {
            frame(&ctx, &mut app, vec![Event::PointerMoved(Pos2::new(560.0 + 13.0 * i as f32, 260.0 + 9.0 * i as f32))]);
        }
        app.dialog = grido::state::Dialog::None;
    }
}

/// A formula typed into a cell, with the function suggestions up, then
/// taken; and the formula bar clicked into.
#[test]
fn a_formula_can_be_typed() {
    let ctx = Context::default();
    let mut app = GridoApp::new(None);
    app.claude = Some(false);
    frame(&ctx, &mut app, vec![]);
    frame(&ctx, &mut app, click(Pos2::new(200.0, 200.0)));
    for text in ["=", "s", "u"] {
        frame(&ctx, &mut app, vec![Event::Text(text.into())]);
        frame(&ctx, &mut app, vec![Event::PointerMoved(Pos2::new(210.0, 230.0))]);
    }
    let key = |key| Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE };
    frame(&ctx, &mut app, vec![key(egui::Key::ArrowDown)]);
    frame(&ctx, &mut app, vec![key(egui::Key::Tab)]);
    frame(&ctx, &mut app, vec![Event::Text("1,2)".into())]);
    frame(&ctx, &mut app, vec![key(egui::Key::Enter)]);
    frame(&ctx, &mut app, click(Pos2::new(700.0, 108.0)));
    frame(&ctx, &mut app, vec![Event::Text("7".into())]);
    frame(&ctx, &mut app, vec![key(egui::Key::Enter)]);
    frame(&ctx, &mut app, vec![]);
}
