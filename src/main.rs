#[macro_use]
extern crate lazy_static;

use chrono::{Local, NaiveDate};
use clap::Parser;
use color_eyre::Section;
use ratatui::layout::Rows;
use std::fmt;
use std::fs::{create_dir, read_to_string};
use std::io::Write;
use std::path::Path;
use std::time::Duration;
use std::{fs::File, path::PathBuf};

#[cfg(debug_assertions)]
use std::str::FromStr;

use color_eyre::{eyre::WrapErr, Result};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::prelude::*;
use ratatui::widgets::block::*;
use ratatui::widgets::*;

mod errors;
mod keybinds;
mod palette;
mod tui;

// Find the Excalidraw file for plans for this app.

#[derive(Parser)]
struct Cli {
    /// Prints tasks to stdout.
    #[arg(long = "simple", short = 's')]
    simple: bool,

    /// Load the master task list.
    #[arg(long = "master", short = 'm')]
    master: bool,

    /// Load Yesterdays date (effects --date too).
    #[arg(long = "yesterday", short = 'y')]
    yesterday: bool,

    /// Load a specific date, format must be DD-MM-YY.
    #[arg(long = "date", short = 'd')]
    date: Option<String>,
}

enum LoadTarget {
    Master,
    Date(NaiveDate),
}

fn main() -> Result<()> {
    // color_eyre::install()?;
    let args = Cli::parse();
    let mut date = Local::now().date_naive();

    if let Some(d) = args.date {
        date = NaiveDate::parse_from_str(&d, "%d-%m-%y")
            .wrap_err("Unable to convert input str to Date object")
            .suggestion("Should follow day-month-year E.G. 09-03-01")?;
    }

    if args.yesterday {
        date = date.pred_opt().expect("Should be vaild date!");
    }

    let target = match args.master {
        true => LoadTarget::Master,
        false => LoadTarget::Date(date),
    };

    let file: PathBuf = get_target_file(&target)?;

    let mut data = load_data(&file)?;

    #[cfg(debug_assertions)]
    if data.is_empty() {
        data.push(Task::new("This is a test string!"));
        data.push(Task::new("This is Awesome, Though just a test!"));
        data[0].toggle_completed();
    }

    match args.simple {
        true => simple_stdout_print(&data),
        false => run_tui(&mut data, &file)?,
    }

    save_data(&file, data)?;
    Ok(())
}

fn run_tui(data: &mut Vec<Task>, file: &Path) -> Result<()> {
    errors::install_hooks()?;
    let mut terminal = tui::init()?;
    let app_result = App::from(data, file).run(&mut terminal);
    tui::restore()?;
    app_result
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum EventState {
    Normal,
    NewTask,
    SelectTask,
    ChangePriority,
}

#[derive(Debug)]
struct App<'a> {
    data: &'a mut Vec<Task>,
    selected: Option<usize>,
    event_state: EventState,
    text_buf: String,
    show_keybinds: bool,
    current_file: PathBuf,
    exit: bool,
}

impl<'a> App<'a> {
    fn from(data: &'a mut Vec<Task>, current_file: &Path) -> Self {
        App {
            data,
            selected: None,
            event_state: EventState::Normal,
            text_buf: String::new(),
            show_keybinds: false,
            current_file: current_file.to_path_buf(),
            exit: false,
        }
    }

    fn run(&mut self, terminal: &mut tui::Tui) -> Result<()> {
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
        if event::poll(Duration::from_millis(100))? {
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
        if self.event_state != EventState::Normal {
            match key_event.code {
                KeyCode::Esc => {
                    self.text_buf = String::new();
                    self.event_state = EventState::Normal;
                }
                KeyCode::Backspace => {
                    self.text_buf.pop();
                }
                KeyCode::Enter => {
                    match self.event_state {
                        EventState::NewTask => self.data.push(Task::new(&self.text_buf)),
                        EventState::ChangePriority => {
                            self.change_task_priority(self.text_buf.parse().ok())
                        }
                        EventState::SelectTask => {
                            self.selected = match self.text_buf.parse() {
                                Ok(num) => match num >= self.data.len() {
                                    true => None,
                                    false => Some(num),
                                },
                                Err(_) => None,
                            }
                        }
                        _ => {}
                    };
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
        } else {
            match key_event.code {
                KeyCode::Char('q') => self.exit = true,
                KeyCode::Char('n') => self.event_state = EventState::NewTask,
                KeyCode::Char('k') => self.show_keybinds = !self.show_keybinds,
                KeyCode::Char('s') => self.event_state = EventState::SelectTask,
                KeyCode::Char('D') => self.delete_task(),
                KeyCode::Char('t') => self.toggle_task(),
                KeyCode::Char('p') => self.event_state = EventState::ChangePriority,
                KeyCode::Esc => self.selected = None,
                _ => {}
            }
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
                Constraint::Max(8),
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

        // Color the background.
        Block::default().bg(palette::BACKGROUND).render(area, buf);

        // Draw header objects.
        Paragraph::new(title).render(header_layout[0], buf);

        let mut input_container_block = Block::default();
        let input_container_block_inner = input_container_block.inner(header_layout[1]);
        let padding = || {
            if input_container_block_inner.width <= 80 {
                return 0;
            } else {
                return (input_container_block_inner.width - 80) / 2;
            }
        };
        input_container_block = input_container_block.padding(Padding::horizontal(padding()));

        if self.event_state != EventState::Normal {
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
            .border_style(Style::default().fg(palette::BASE_1))
            .borders(Borders::TOP)
            .title_style(Style::default().fg(palette::PURPLE))
            .render(outer_layout[1], buf);

        let mut rows = Rows::new(task_layout);
        self.data.iter().enumerate().for_each(|(index, task)| {
            let (priority, desc) = task.get_data();
            let mut task_widget = Line::from(vec![
                Span::from(format!("{: >4}", index)).fg(palette::WHITE),
                Span::from(format!("{: ^8}", priority)).fg(palette::ORANGE),
                Span::from(desc).fg(palette::YELLOW),
            ]);
            if let Some(n) = self.selected {
                if n == index {
                    task_widget = task_widget.bg(palette::BASE_2);
                }
            }
            if task.completed {
                task_widget = task_widget.add_modifier(Modifier::CROSSED_OUT);
            }
            if let Some(row) = rows.next() {
                task_widget.render(row, buf);
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

#[derive(Debug, Clone)]
struct Task {
    priority: i8,
    completed: bool,
    task: String,
}

#[allow(dead_code)]
impl Task {
    fn new(task: &str) -> Task {
        Task {
            priority: 0,
            completed: false,
            task: task.to_string(),
        }
    }
    fn set_priority(&mut self, new_priority: i8) {
        self.priority = new_priority;
    }
    fn toggle_completed(&mut self) {
        self.completed = !self.completed;
    }
    fn get_data(&self) -> (i8, String) {
        (self.priority, self.task.clone())
    }
}

impl fmt::Display for Task {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{: >8} | {: ^9} | {}",
            self.priority, self.completed, self.task
        )
    }
}

fn simple_stdout_print(data: &[Task]) {
    println!("Priority | Completed | Task");
    data.iter().for_each(|task| println!("{}", task));
}

fn get_target_file(target: &LoadTarget) -> Result<PathBuf> {
    let mut path = home::home_dir().expect("Should find home directory");
    path.push(".local/share/todo-rs/");

    #[cfg(debug_assertions)]
    let mut path = PathBuf::from_str("./todo-rs/")?;

    match target {
        LoadTarget::Date(date) => {
            path.push(format!("{}.todo", date.format("%d-%m-%y")));
        }
        LoadTarget::Master => path.push("master.todo"),
    }

    Ok(path)
}

fn load_data(file: &PathBuf) -> Result<Vec<Task>> {
    if !file.parent().unwrap().exists() {
        create_dir(file.parent().unwrap())?;
    }

    let input = match read_to_string(file) {
        Ok(x) => x,
        // Err(NotFound) => return Ok(Vec::new()),
        // Err(e) => return Err(e.into()),
        Err(error) => match error.kind() {
            std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            _ => return Err(error.into()),
        },
    };

    let data = input
        .split('\n')
        .filter_map(|line| {
            if line.is_empty() {
                return None;
            }

            let (priority, blob) = line.split_once(',').expect("Should find priority");
            let (completed, task) = blob.split_once(',').expect("Should find Complted/Task");

            Some(Task {
                priority: priority.parse().expect("Should be Digit!"),
                completed: completed.parse().expect("Should be a bool!"),
                task: task.to_string(),
            })
        })
        .collect::<Vec<Task>>();

    Ok(data)
}

fn save_data(file: &PathBuf, data: Vec<Task>) -> Result<()> {
    if !file.parent().unwrap().exists() {
        create_dir(file.parent().unwrap())?;
    }
    let mut output = File::create(file)?;

    for task in data {
        writeln!(output, "{},{},{}", task.priority, task.completed, task.task)?;
    }

    Ok(())
}
