//! `grido`: the spreadsheet window, and the MCP server an assistant reaches
//! it through. The arguments decide which.

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    grido_app::app::run(&args)
}
