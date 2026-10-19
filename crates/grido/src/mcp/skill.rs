//! The guide in Grido's skill: how to work with the spreadsheet.
//!
//! Tool schemas say what each tool takes; they are a poor place for *how to
//! work*. This guide carries that: which tool to reach for, the record-oriented
//! habit that keeps an assistant from guessing at coordinates, and what Grido
//! does behind the scenes (backups, autosave, confirmations) so the assistant
//! doesn't reinvent them. The skill file is written by [`crate::claude`], with
//! this guide in it.

/// The guide, as it goes into the skill.
pub const GUIDE: &str = r#"# Working with Grido

Grido is a native spreadsheet that is already open on the user's screen. Your
edits appear there immediately and are briefly highlighted, so the user watches
you work. Treat their file with corresponding care.

## Start by looking

Call `grido_workbook_info` first. It gives you the file path, every sheet, its
used range and its header row. Then `grido_table_schema` for the sheet you will
work on. Never assume a layout — a sheet the user has kept for months will have
grown columns you did not put there.

## Prefer records over coordinates

Sheets that hold a list get record-shaped tools. Use them:

- `grido_table_append` with a `{column name: value}` record adds a row in the
  right place, filling only the columns you name.
- `grido_table_find` with criteria locates rows.
- `grido_table_update` changes the rows matching criteria.

These match against the header row, so "the Acme row" resolves correctly even
after sorting or inserted rows. Reaching for `grido_cell_set` with a computed
row number is how mistakes happen — use it for one-off cells outside a table,
or for headers and formulas.

`grido_table_update` refuses to touch several rows at once unless you pass
`all: true`. That guard exists because a vague criterion would otherwise
rewrite the whole table; if it fires, narrow the criteria rather than forcing
it, and say what you found.

## Reading without drowning

`grido_range_read` is capped. Ask for the range you need. To answer questions
about totals or counts, prefer `grido_stats` over reading every row, and let the
sheet's own formulas do the work — they recalculate as you write.

## Building something new

`grido_template_create` produces a ready formatted workbook (`job-applications`,
`budget`, `contacts`): headers, number formats, a frozen header row and
summary formulas. Start there instead of assembling a sheet cell by cell, then
adapt it — add columns with `grido_cell_set` on the header row and
`grido_range_format` for the column beneath.

Give it a `path` so the file exists somewhere the user can find again.

## Formulas

Write formulas, not computed constants: `=SUM(D2:D100)` rather than a total you
worked out yourself. The file stays correct after the user edits it by hand.
Anything starting with `=` in `grido_cell_set` or a record value is evaluated.

## What Grido already handles

Do not build these yourself:

- **Undo** — one tool call is one Ctrl+Z for the user.
- **Backups** — the file is copied before your first write of a session.
- **Saving** — work is saved automatically a few seconds after you stop. Call
  `grido_workbook_save` only when the user asks, or to save somewhere new.
- **Destructive actions** — `grido_rows_delete` asks the user in the app and
  only proceeds if they approve. Say that you have asked; do not wait for a
  result that will not come back.

## When something is refused

Errors are written for you to act on. "No column named X (have: …)" means use
one of the listed columns or add the new one deliberately. "The user is editing
a cell right now" means wait a moment and retry. Do not work around a refusal
by writing raw cells.

## Talking to the user

They can see the sheet. Describe what changed in their terms — "marked Globex
as an offer and noted the Friday deadline" — not in cell references.
"#;

/// The same guidance, condensed, for MCP's `initialize` response — clients
/// that have no notion of skills still get the important parts.
pub fn instructions() -> &'static str {
    "Grido is a spreadsheet open on the user's screen; your edits appear there \
     live. Call grido_workbook_info first, then grido_table_schema for the \
     sheet you will touch. For lists, use grido_table_append / \
     grido_table_find / grido_table_update with {column name: value} records \
     rather than computing row numbers — they match against the header row. \
     Write formulas rather than computed constants. Grido handles undo \
     grouping, backups, autosave and confirmation of destructive actions, so \
     do not reimplement them. Errors are actionable: read them and adjust \
     rather than falling back to raw cell writes."
}
