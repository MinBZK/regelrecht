use crate::backend::process_runner::{ProcessMessage, ProcessMessageKind, ProcessRunner};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{
    Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
};

const MAX_OUTPUT_LINES: usize = 10_000;

struct ActionCommand {
    key: char,
    name: &'static str,
    just_target: &'static str,
}

const COMMANDS: &[ActionCommand] = &[
    ActionCommand {
        key: 'f',
        name: "Format",
        just_target: "format",
    },
    ActionCommand {
        key: 'l',
        name: "Lint",
        just_target: "lint",
    },
    ActionCommand {
        key: 'b',
        name: "Build check",
        just_target: "build-check",
    },
    ActionCommand {
        key: 'v',
        name: "Validate",
        just_target: "validate",
    },
    ActionCommand {
        key: 'c',
        name: "Check (all)",
        just_target: "check",
    },
    ActionCommand {
        key: 't',
        name: "Test",
        just_target: "test",
    },
    ActionCommand {
        key: 'a',
        name: "Test all",
        just_target: "test-all",
    },
    ActionCommand {
        key: 'd',
        name: "BDD",
        just_target: "bdd",
    },
];

pub struct ActionsView {
    output: Vec<OutputLine>,
    scroll_offset: usize,
    running: Option<String>,
    last_exit_code: Option<Option<i32>>,
}

#[derive(Clone)]
struct OutputLine {
    text: String,
    is_stderr: bool,
}

impl ActionsView {
    pub fn new() -> Self {
        Self {
            output: Vec::new(),
            scroll_offset: 0,
            running: None,
            last_exit_code: None,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent, runner: &mut ProcessRunner) {
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => self.scroll_down(1),
            KeyCode::Up | KeyCode::Char('k') => self.scroll_up(1),
            KeyCode::Char('G') => self.scroll_to_bottom(),
            KeyCode::Char('g') => self.scroll_offset = 0,
            KeyCode::Char(c) => {
                if runner.is_running() {
                    return;
                }
                if let Some(cmd) = COMMANDS.iter().find(|cmd| cmd.key == c) {
                    self.output.clear();
                    self.scroll_offset = 0;
                    self.last_exit_code = None;
                    self.running = Some(cmd.name.to_string());
                    let task_id = format!("action:{}", cmd.just_target);
                    runner.run_just(task_id, cmd.just_target);
                }
            }
            _ => {}
        }
    }

    pub fn handle_process_message(&mut self, msg: ProcessMessage) {
        match msg.kind {
            ProcessMessageKind::Stdout(line) => {
                self.push_output(line, false);
            }
            ProcessMessageKind::Stderr(line) => {
                self.push_output(line, true);
            }
            ProcessMessageKind::Done { exit_code } => {
                self.last_exit_code = Some(exit_code);
                self.running = None;
            }
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let layout = Layout::vertical([
            Constraint::Length(COMMANDS.len() as u16 + 2),
            Constraint::Min(0),
        ])
        .split(area);

        self.render_commands(frame, layout[0]);
        self.render_output(frame, layout[1]);
    }

    fn render_commands(&self, frame: &mut Frame, area: Rect) {
        let dim = Style::default().add_modifier(Modifier::DIM);
        let mut lines = Vec::new();

        for cmd in COMMANDS {
            let is_running = self.running.as_ref().is_some_and(|name| name == cmd.name);

            let indicator = if is_running { "● " } else { "  " };
            let key_style = Style::default().add_modifier(Modifier::BOLD);
            let name_style = if is_running {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                dim
            };

            lines.push(Line::from(vec![
                Span::styled(indicator, dim),
                Span::styled(format!("[{}]", cmd.key), key_style),
                Span::raw(" "),
                Span::styled(cmd.name, name_style),
                Span::styled(
                    format!("  just {}", cmd.just_target),
                    Style::default().add_modifier(Modifier::DIM),
                ),
            ]));
        }

        let block = Block::default()
            .title(Span::styled(
                " Quick Actions ",
                Style::default().add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL);

        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
    }

    fn render_output(&self, frame: &mut Frame, area: Rect) {
        let inner_height = area.height.saturating_sub(2) as usize;
        let total_lines = self.output.len();
        let dim = Style::default().add_modifier(Modifier::DIM);

        // Clamp scroll offset so we always show a full page when possible
        let max_scroll = total_lines.saturating_sub(inner_height);
        let effective_scroll = self.scroll_offset.min(max_scroll);

        let visible_lines: Vec<Line> = self
            .output
            .iter()
            .skip(effective_scroll)
            .take(inner_height)
            .map(|line| {
                let style = if line.is_stderr {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    dim
                };
                Line::styled(&*line.text, style)
            })
            .collect();

        let title = match (&self.running, self.last_exit_code) {
            (Some(name), _) => format!(" ● Running: {name} "),
            (None, Some(Some(0))) => " ✓ Done ".to_string(),
            (None, Some(Some(code))) => format!(" ✗ Exit code: {code} "),
            (None, Some(None)) => " ✗ Killed ".to_string(),
            (None, None) => " Output ".to_string(),
        };

        let block = Block::default()
            .title(Span::styled(
                title,
                Style::default().add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL);

        let paragraph = Paragraph::new(visible_lines)
            .block(block)
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);

        if total_lines > inner_height {
            let mut scrollbar_state = ScrollbarState::new(max_scroll).position(effective_scroll);
            frame.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight),
                area.inner(Margin {
                    vertical: 1,
                    horizontal: 0,
                }),
                &mut scrollbar_state,
            );
        }
    }

    fn scroll_down(&mut self, n: usize) {
        let max = self.output.len().saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + n).min(max);
    }

    fn scroll_up(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
    }

    fn scroll_to_bottom(&mut self) {
        // Scroll to end; render will clamp to actual viewport height
        self.scroll_offset = self.output.len().saturating_sub(1);
    }

    fn push_output(&mut self, text: String, is_stderr: bool) {
        self.output.push(OutputLine { text, is_stderr });
        if self.output.len() > MAX_OUTPUT_LINES {
            self.output.drain(..self.output.len() - MAX_OUTPUT_LINES);
            self.scroll_offset = self.scroll_offset.saturating_sub(1);
        }
        self.scroll_to_bottom();
    }
}
