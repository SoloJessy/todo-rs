#[macro_use]
extern crate lazy_static;

use chrono::{Local, NaiveDate};
use clap::Parser;
use std::fmt;
use std::fs::File;
use std::fs::{create_dir, read_to_string};
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use color_eyre::{eyre::WrapErr, Result, Section};

mod app;
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

    #[arg(exclusive = true, long = "version", short = 'v')]
    version: bool,
}

enum LoadTarget {
    Master,
    Date(NaiveDate),
}

fn main() -> Result<()> {
    // color_eyre::install()?;
    let args = Cli::parse();

    if args.version {
        println!(
            "App {} version: {}",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION")
        );
        return Ok(());
    }

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
    let app_result = app::App::from(data, file).run(&mut terminal);
    tui::restore()?;
    app_result
}

#[derive(Debug, Clone)]
pub struct Task {
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
    let mut path = PathBuf::from("./todo-rs/");

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
