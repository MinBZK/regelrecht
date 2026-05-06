//! View-side helpers shared across TUI panels.

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders};

/// Build the bordered, bold-title block used as the chrome of every TUI panel.
///
/// Every panel previously hand-rolled
/// `Block::default().borders(Borders::ALL).title(Span::styled(<title>,
/// Style::default().add_modifier(Modifier::BOLD)))`. This helper is the single
/// source of truth so panel chrome stays consistent.
pub fn titled_block(title: &str) -> Block<'_> {
    Block::default().borders(Borders::ALL).title(Span::styled(
        title,
        Style::default().add_modifier(Modifier::BOLD),
    ))
}
