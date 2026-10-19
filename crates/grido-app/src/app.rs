use std::path::PathBuf;

use eframe::egui;
use crate::role::{self, Role};
use crate::{host, mcp};
use grido::state::GridoApp;

/// Runs the role a command line asks for.
pub fn run(args: &[String]) -> eframe::Result {
    match role::of(args) {
        // The stdio shim an MCP client launches. It answers the handshake and
        // the read-only tools itself and opens nothing; anything that acts goes
        // to the server, which is the only place the user can be asked first.
        Role::Shim => {
            if let Err(e) = mcp::proto::stdio_shim() {
                eprintln!("grido mcp: {e}");
                std::process::exit(1);
            }
            Ok(())
        }

        // The server: the socket, the registry, the confirmation broker. No
        // window of its own, so it needs no display to run.
        Role::Serve => {
            crate::serve::run();
            Ok(())
        }

        // One confirmation dialog, for the question on stdin.
        Role::Confirm => crate::dialog::run(),

        Role::Window { product, path } => window(product, path),
    }
}

/// Shows a product — or finds that another window already is, and hands its
/// file over instead of opening a second one.
fn window(product: String, path: Option<PathBuf>) -> eframe::Result {
    if product != role::DEFAULT_PRODUCT {
        eprintln!("grido: there is no product called \"{product}\"");
        std::process::exit(2);
    }
    let link = match host::start(&product, path.as_deref()) {
        host::Outcome::HandedOff => return Ok(()),
        host::Outcome::Run(link) => link,
    };

    // When the user let Claude use Grido, keep that registration pointing at
    // this binary.
    std::thread::spawn(grido::claude::refresh);

    let options = eframe::NativeOptions {
        // A Wayland window that is not on screen gets no frame callbacks, and
        // a vsynced swap waits for one: the loop would stop answering the
        // compositor, which then calls Grido not responding. egui only paints
        // when something changed, so nothing is lost without it.
        vsync: false,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title("Grido")
            .with_app_id("grido"),
        ..Default::default()
    };
    eframe::run_native(
        "grido",
        options,
        Box::new(move |cc| {
            omarchy_kit::fonts::install(&cc.egui_ctx);
            // Wear the Omarchy theme from the first frame.
            cc.egui_ctx
                .set_visuals(omarchy_kit::theme::visuals(&omarchy_kit::theme::load()));
            let mut app = GridoApp::new(path);

            // The bridge carries a tool call from the socket thread onto this
            // one, where the workbook lives. It must exist before the server is
            // told we are here, so start it first.
            let rx = mcp::bridge::install();
            mcp::bridge::set_wake(cc.egui_ctx.clone());
            app.mcp_rx = Some(rx);
            app.mcp_socket = Some(mcp::proto::socket_path());
            app.mcp_serving = link.is_some();
            if let Some(link) = link {
                host::serve(product, link);
            }

            Ok(Box::new(app))
        }),
    )
}
