use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{
    Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
};
use std::collections::VecDeque;

const MAX_LINES: usize = 10_000;

#[derive(Clone, Copy, PartialEq)]
enum LogLevel {
    All,
    Error,
    Warn,
    Info,
    Debug,
}

impl LogLevel {
    fn label(&self) -> &'static str {
        match self {
            LogLevel::All => "all",
            LogLevel::Error => "error",
            LogLevel::Warn => "warn+",
            LogLevel::Info => "info+",
            LogLevel::Debug => "debug+",
        }
    }
}

pub struct LogsView {
    lines: VecDeque<String>,
    filter: LogLevel,
    follow: bool,
    scroll_offset: usize,
}

impl LogsView {
    pub fn new() -> Self {
        Self {
            lines: VecDeque::new(),
            filter: LogLevel::All,
            follow: true,
            scroll_offset: 0,
        }
    }

    #[allow(dead_code)]
    pub fn push_line(&mut self, line: String) {
        self.lines.push_back(line);
        if self.lines.len() > MAX_LINES {
            self.lines.pop_front();
        }
        if self.follow {
            self.scroll_to_bottom();
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
                self.follow = false;
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
                self.follow = false;
            }
            KeyCode::Char('g') => {
                self.scroll_offset = 0;
                self.follow = false;
            }
            KeyCode::Char('G') => {
                self.scroll_to_bottom();
                self.follow = true;
            }
            KeyCode::Char('f') => {
                self.follow = !self.follow;
                if self.follow {
                    self.scroll_to_bottom();
                }
            }
            KeyCode::Char('e') => self.filter = LogLevel::Error,
            KeyCode::Char('w') => self.filter = LogLevel::Warn,
            KeyCode::Char('i') => self.filter = LogLevel::Info,
            KeyCode::Char('d') => self.filter = LogLevel::Debug,
            KeyCode::Char('a') => self.filter = LogLevel::All,
            _ => {}
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let inner_height = area.height.saturating_sub(2) as usize;

        let filtered: Vec<&str> = self
            .lines
            .iter()
            .filter(|l| matches_filter(l, &self.filter))
            .map(|l| l.as_str())
            .collect();

        let total = filtered.len();
        let max_scroll = total.saturating_sub(inner_height);
        let effective_scroll = self.scroll_offset.min(max_scroll);

        let visible: Vec<Line> = filtered
            .iter()
            .skip(effective_scroll)
            .take(inner_height)
            .map(|l| {
                let style = if l.contains("ERROR") || l.contains("error") {
                    Style::default().add_modifier(Modifier::BOLD)
                } else if l.contains("WARN") || l.contains("warn") {
                    Style::default().add_modifier(Modifier::ITALIC)
                } else if l.contains("DEBUG") || l.contains("debug") {
                    Style::default().add_modifier(Modifier::DIM)
                } else {
                    Style::default()
                };
                Line::styled(*l, style)
            })
            .collect();

        let follow_indicator = if self.follow { " [follow]" } else { "" };
        let title = if self.lines.is_empty() {
            format!(
                " Logs — no output yet  filter:{}{}  f:toggle-follow ",
                self.filter.label(),
                follow_indicator
            )
        } else {
            format!(
                " Logs ({} lines)  filter:{}{}  f:toggle-follow ",
                total,
                self.filter.label(),
                follow_indicator
            )
        };

        let block = Block::default().borders(Borders::ALL).title(Span::styled(
            title,
            Style::default().add_modifier(Modifier::BOLD),
        ));

        if self.lines.is_empty() {
            let content = Paragraph::new(vec![
                Line::from(""),
                Line::from("  No log output yet."),
                Line::from(""),
                Line::from(Span::styled(
                    "  Worker logs will appear here when workers are running.",
                    Style::default().add_modifier(Modifier::DIM),
                )),
                Line::from(Span::styled(
                    "  Filter: e=error  w=warn  i=info  d=debug  a=all",
                    Style::default().add_modifier(Modifier::DIM),
                )),
            ])
            .block(block);
            frame.render_widget(content, area);
        } else {
            let paragraph = Paragraph::new(visible).block(block);
            frame.render_widget(paragraph, area);

            if total > inner_height {
                let mut state = ScrollbarState::new(total.saturating_sub(inner_height))
                    .position(self.scroll_offset);
                frame.render_stateful_widget(
                    Scrollbar::new(ScrollbarOrientation::VerticalRight),
                    area.inner(Margin {
                        vertical: 1,
                        horizontal: 0,
                    }),
                    &mut state,
                );
            }
        }
    }

    fn scroll_to_bottom(&mut self) {
        // Scroll to end; render clamps to actual viewport height
        self.scroll_offset = self.lines.len().saturating_sub(1);
    }
}

fn matches_filter(line: &str, filter: &LogLevel) -> bool {
    match filter {
        LogLevel::All => true,
        LogLevel::Error => {
            line.contains("ERROR") || line.contains("error") || line.contains("Error")
        }
        LogLevel::Warn => {
            line.contains("ERROR")
                || line.contains("error")
                || line.contains("WARN")
                || line.contains("warn")
        }
        LogLevel::Info => {
            !line.contains("DEBUG") && !line.contains("debug") && !line.contains("TRACE")
        }
        LogLevel::Debug => !line.contains("TRACE"),
    }
}
