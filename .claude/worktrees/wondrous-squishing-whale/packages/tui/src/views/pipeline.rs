use crossterm::event::KeyEvent;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

pub struct PipelineView {
    connected: bool,
}

impl PipelineView {
    pub fn new() -> Self {
        // TODO: In Phase 9, try connecting to DATABASE_URL
        let connected = false;

        Self { connected }
    }

    pub fn handle_key(&mut self, _key: KeyEvent) {
        // TODO: implement in Phase 9 — refresh, navigate tables
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default().borders(Borders::ALL).title(Span::styled(
            " Pipeline Monitor ",
            Style::default().add_modifier(Modifier::BOLD),
        ));

        if !self.connected {
            let content = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  Not connected",
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from("  Set DATABASE_URL environment variable to connect to the"),
                Line::from("  pipeline database and monitor harvest/enrich jobs."),
                Line::from(""),
                Line::from(Span::styled(
                    "  Example: DATABASE_URL=postgres://localhost/regelrecht rrtui",
                    Style::default().add_modifier(Modifier::DIM),
                )),
            ])
            .block(block);
            frame.render_widget(content, area);
        } else {
            let content = Paragraph::new("  Connected — loading...").block(block);
            frame.render_widget(content, area);
        }
    }
}
