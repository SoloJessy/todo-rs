use std::fmt;

pub struct Kb {
    pub key: Kc,
    pub modifier: Km,
    pub description: String,
}

impl Kb {
    fn new(key: Kc, modifier: Km, description: &str) -> Self {
        Kb {
            key,
            modifier,
            description: description.to_string(),
        }
    }
}

#[derive(Debug)]
pub enum Km {
    None,
    Shift,
}

impl fmt::Display for Km {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let text = match self {
            Km::None => "",
            Km::Shift => "Shift",
        };
        write!(f, "{}", text)
    }
}

pub enum Kc {
    Char(char),
    Esc,
}

impl fmt::Display for Kc {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let text = match self {
            Kc::Char(c) => format!("{}", c),
            Kc::Esc => "Esc".to_string(),
        };
        write!(f, "{}", text)
    }
}

lazy_static! {
    pub static ref KEYBINDS: Vec<Kb> = vec![
        Kb::new(Kc::Char('n'), Km::None, "Create new Task."),
        Kb::new(Kc::Char('s'), Km::None, "Select Task."),
        Kb::new(Kc::Char('p'), Km::None, "Change Task Priority."),
        Kb::new(Kc::Char('t'), Km::None, "Toggle Task Completion."),
        Kb::new(Kc::Char('D'), Km::Shift, "Delete Task."),
        Kb::new(Kc::Char('h'), Km::None, "Split completed tasks."),
        Kb::new(Kc::Esc, Km::None, "Clear Selected / Cancel Input."),
        Kb::new(Kc::Char('k'), Km::None, "Show this Modal."),
        Kb::new(Kc::Char('q'), Km::None, "Quit the App.",),
    ];
}
