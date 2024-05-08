use color_eyre::{eyre::WrapErr, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::Rows;
use ratatui::prelude::*;
use ratatui::widgets::block::*;
use ratatui::widgets::*;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::{keybinds, palette, tui, Task};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum EventState {
    Normal,
    TextInput,
}

#[derive(Debug)]
pub struct App<'a> {
    data: &'a mut Vec<Task>,
    selected: Option<usize>,
    event_state: EventState,
    text_buf: String,
    show_keybinds: bool,
    split_tasks: bool,
    current_file: PathBuf,
    exit: bool,
}

impl<'a> App<'a> {
    pub fn from(data: &'a mut Vec<Task>, current_file: &Path) -> Self {
        App {
            data,
            selected: None,
            event_state: EventState::Normal,
            text_buf: String::new(),
            show_keybinds: false,
            split_tasks: false,
            current_file: current_file.to_path_buf(),
            exit: false,
        }
    }

    pub fn run(&mut self, terminal: &mut tui::Tui) -> Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.render_frame(frame))?;
            self.handle_events().wrap_err("handle events failed")?;
        }
        Ok(())
    }

    fn render_frame(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.size());
    }

    fn handle_events(&mut self) -> Result<()> {
        if event::poll(Duration::from_millis(1000))? {
            match event::read()? {
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                    self.handle_key_press_event(key_event)
                }
                _ => {}
            };
        }
        Ok(())
    }

    fn handle_key_press_event(&mut self, key_event: KeyEvent) {
        if self.event_state == EventState::TextInput {
            match key_event.code {
                KeyCode::Esc => {
                    self.text_buf = String::new();
                    self.event_state = EventState::Normal;
                }
                KeyCode::Backspace => {
                    self.text_buf.pop();
                }
                KeyCode::Enter => {
                    self.data.push(Task::new(&self.text_buf));
                    self.event_state = EventState::Normal;
                    self.text_buf = String::new();
                }
                KeyCode::Char(value) => {
                    if self.text_buf.len() <= 80 {
                        self.text_buf.push(value);
                    }
                }
                _ => {}
            }
            return;
        }
        match key_event.code {
            KeyCode::Char(x) if x.is_ascii_digit() => self.text_buf.push(x),
            KeyCode::Char('q') => self.exit = true,
            KeyCode::Char('n') => {
                self.text_buf = String::new();
                self.event_state = EventState::TextInput;
            }
            KeyCode::Char('k') => self.show_keybinds = !self.show_keybinds,
            KeyCode::Char('s') => {
                // self.event_state = EventState::SelectTask
                self.selected = match self.text_buf.parse() {
                    Ok(num) => match num >= self.data.len() {
                        true => None,
                        false => Some(num),
                    },
                    Err(_) => None,
                };
                self.text_buf = String::new();
            }
            KeyCode::Char('D') => self.delete_task(),
            KeyCode::Char('t') => self.toggle_task(),
            KeyCode::Char('p') => {
                self.change_task_priority(self.text_buf.parse().ok());
                self.text_buf = String::new();
            }
            KeyCode::Char('h') => self.split_tasks = !self.split_tasks,
            KeyCode::Esc => {
                self.selected = None;
                self.text_buf = String::new();
            }
            _ => {}
        }
    }

    fn delete_task(&mut self) {
        if let Some(i) = self.selected {
            self.data.remove(i);
        }
        self.selected = None;
    }

    fn toggle_task(&mut self) {
        if let Some(i) = self.selected {
            self.data[i].toggle_completed();
        }
    }

    fn change_task_priority(&mut self, value: Option<i8>) {
        if let (Some(n), Some(i)) = (value, self.selected) {
            self.data[i].set_priority(n);
        }
    }
}
// Palette

impl Widget for &App<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // header, body <- layout
        let outer_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Length(1), Constraint::Min(10)])
            .split(area);

        let title = Span::from(" Todo-rs ").fg(palette::WHITE);
        let keybind_reminder = Line::from(vec![
            Span::from(" <K>").fg(palette::LIGHT_BLUE),
            Span::from(" for Keybindings ").fg(palette::WHITE),
        ]);

        // title, text input, keybind reminder <- layout
        let header_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Length(title.width() as u16),
                Constraint::Fill(1),
                Constraint::Length(keybind_reminder.width() as u16),
            ])
            .split(outer_layout[0]);

        let input = Span::from(&self.text_buf).fg(palette::WHITE);

        let keybind_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Min(2),
                Constraint::Max(keybinds::KEYBINDS.len() as u16),
                Constraint::Min(1),
            ])
            .split(
                Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints(vec![
                        Constraint::Min(1),
                        Constraint::Length(47),
                        Constraint::Min(1),
                    ])
                    .split(outer_layout[1])[1],
            )[1];

        let task_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Length(1), Constraint::Min(1)])
            .split(outer_layout[1])[1];

        let split_task_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(task_layout);

        // Color the background.
        Block::default().bg(palette::BACKGROUND).render(area, buf);

        // Draw header objects.
        Paragraph::new(title).render(header_layout[0], buf);

        let mut input_container_block = Block::default();
        let input_container_block_inner = input_container_block.inner(header_layout[1]);

        let padding = || {
            if input_container_block_inner.width <= 80 {
                return 0;
            }
            (input_container_block_inner.width - 80) / 2
        };

        input_container_block = input_container_block.padding(Padding::horizontal(padding()));

        if self.event_state == EventState::TextInput {
            Paragraph::new(input)
                .block(Block::default().bg(palette::BASE_1))
                .render(input_container_block.inner(header_layout[1]), buf)
        }

        Paragraph::new(keybind_reminder).render(header_layout[2], buf);

        // Render data section.
        Block::default()
            .title(format!(
                " {} ",
                self.current_file.file_name().unwrap().to_str().unwrap()
            ))
            .title_alignment(Alignment::Right)
            .border_style(Style::default().fg(palette::BASE_1))
            .borders(Borders::TOP)
            .title_style(Style::default().fg(palette::PURPLE))
            .render(outer_layout[1], buf);

        let priority_color = |x: i8| {
            if x < 50 {
                palette::GREEN
            } else if x < 100 {
                palette::ORANGE
            } else {
                palette::RED
            }
        };

        let mut rows = Rows::new(split_task_layout[0]);
        let mut completed_rows = Rows::new(split_task_layout[1]);

        if !self.split_tasks {
            rows = Rows::new(task_layout);
        }

        self.data.iter().enumerate().for_each(|(index, task)| {
            let (priority, desc) = task.get_data();

            let mut task_widget = Line::from(vec![
                Span::from(format!("{: >4}", index)).fg(palette::WHITE),
                Span::from(format!("{: ^8}", priority)).fg(priority_color(priority)),
                Span::from(desc).fg(palette::YELLOW),
            ]);

            if let Some(n) = self.selected {
                if n == index {
                    task_widget = task_widget.bg(palette::BASE_1);
                }
            }

            if task.completed {
                task_widget.spans[2].style.fg = Some(palette::BASE_2);
            }

            let render = |rows: &mut Rows| {
                if let Some(row) = rows.next() {
                    task_widget.render(row, buf);
                }
            };

            if self.split_tasks {
                if task.completed {
                    render(&mut completed_rows);
                } else {
                    render(&mut rows);
                }
            } else {
                render(&mut rows);
            }
        });

        if self.show_keybinds {
            Clear.render(keybind_layout, buf);

            Block::default()
                .bg(palette::BASE_1)
                .render(keybind_layout, buf);

            let mut rows = Rows::new(keybind_layout);

            keybinds::KEYBINDS.iter().for_each(|kb| {
                let binds_text = Line::from(vec![
                    Span::from(format!("{:^5}", kb.key.to_string())).fg(palette::LIGHT_BLUE),
                    Span::from("|").fg(palette::WHITE),
                    Span::from(format!("{:^8}", kb.modifier.to_string())).fg(palette::LIGHT_BLUE),
                    Span::from("|").fg(palette::WHITE),
                    Span::from(format!(" {}", kb.description)).fg(palette::YELLOW),
                ]);

                if let Some(row) = rows.next() {
                    binds_text.render(row, buf)
                }
            })
        }
    }
}
