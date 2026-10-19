//! The spreadsheet's MCP surface.
//!
//! The transport lives in `grido-app` and is tested there; what matters
//! here is that the tools and the skill are ones a model can act on.

/// Every advertised tool must have a name, a description and an object schema
/// — a tool the model cannot understand is worse than no tool.
#[test]
fn tool_definitions_are_well_formed() {
    let tools = grido::mcp::tools::definitions();
    assert!(tools.len() >= 15, "expected a useful tool surface");
    for tool in &tools {
        let name = tool["name"].as_str().expect("name");
        // One server carries every product, so a name says which one it is.
        assert!(
            name.starts_with(grido::mcp::tools::PREFIX),
            "{name} must be named grido_<verb>"
        );
        let desc = tool["description"].as_str().expect("description");
        assert!(
            desc.len() > 20,
            "{name} needs a description the model can act on"
        );
        assert_eq!(tool["inputSchema"]["type"], "object", "{name} schema");
        assert!(
            tool["inputSchema"]["properties"].is_object(),
            "{name} properties"
        );
    }
}

/// The guide in Grido's skill says where to start and how to write.
#[test]
fn the_guide_says_where_to_start() {
    let guide = grido::mcp::skill::GUIDE;
    assert!(guide.starts_with("# Working with Grido"));
    assert!(guide.contains("grido_workbook_info"));
    assert!(guide.contains("grido_table_append"));
}
