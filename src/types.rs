use std::error::Error;
use std::fmt;
use std::str::FromStr;
use regex::Regex;
use crate::consts;

pub struct CodeBlock {
    pub lines: Vec<String>,
    pub start: usize,
}

impl CodeBlock {
    pub fn format(&self) -> String {
        let max_line = self.start + self.lines.len();
        let padding = max_line.to_string().len();

        self.lines.iter()
            .enumerate()
            .map(|(i, line)| {
                format!("{:width$} | {}", self.start + i + 1, line, width = padding) // Print non-zero-based
            })
            .collect::<Vec<String>>()
            .join("\n")
    }
}

impl Clone for CodeBlock {
    fn clone(&self) -> Self {
        CodeBlock {
            lines: self.lines.clone(),
            start: self.start,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Command {
    SearchFiles {
        keywords: Vec<String>,
    },
    ReadLines {
        file: String,
        start: usize,
        n: usize,
    },
    Jump {
        file: String,
        line: usize,
        char: usize,
        n: usize,
    },
}

impl Command {
    pub fn match_line(line: &str) -> bool {
        Regex::new(r#"^`?!(\w+) (?:"([^"]+)"*(?: |$|`$))*"#)
            .unwrap()
            .is_match(line)
    }
    pub fn serialize(&self) -> String {
        match self {
            Command::SearchFiles { keywords } => format!(
                r#"!SEARCH_FILES "{}"#,
                keywords
                    .iter()
                    .map(|v| format!(r#""{}""#, v))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            Command::ReadLines { file, start, n } => {
                format!(r#"!READ_LINES "{}" "{}" "{}""#, file, start, n)
            }
            Command::Jump { file, line, char, n } => {
                format!(r#"!JUMP "{}" "{}" "{}" "{}""#, file, line, char, n)
            }
        }
    }
}

impl Clone for Command {
    fn clone(&self) -> Self {
        match self {
            Command::SearchFiles { keywords: query } => Command::SearchFiles {
                keywords: query.clone(),
            },
            Command::ReadLines { file, start, n } => Command::ReadLines {
                file: file.clone(),
                start: *start,
                n: *n,
            },
            Command::Jump { file, line, char, n } => Command::Jump {
                file: file.clone(),
                line: *line,
                char: *char,
                n: *n,
            }
        }
    }
}

impl FromStr for Command {
    type Err = Box<dyn Error>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // let re = Regex::new(r#"^`?! *(\w+)(?: +"([^"]+)")+ *`?$"#).unwrap();

        let re = Regex::new(r#"^`?!(\w+)((?:\s+"[^"]+")*)"#).unwrap();
        let captures = re.captures(s).expect("Invalid command format");
        let name = captures.get(1).map_or("", |m| m.as_str()).to_owned();

        let args_str = captures.get(2).map_or("", |m| m.as_str());
        let re_args = Regex::new(r#""([^"]+)""#).unwrap();
        let args = re_args
            .captures_iter(args_str)
            .map(|c| c.get(1).unwrap().as_str().to_owned())
            .collect::<Vec<_>>();

        match name.as_str() {
            "SEARCH_FILES" => Ok(Command::SearchFiles { keywords: args }),
            "READ_LINES" => {
                if args.len() != 3 {
                    return Err(format!("Expected 3 arguments, got {}", args.len()).into());
                }
                let file = args[0].clone();
                let start = args[1].parse::<usize>().map_err(|e| e.to_owned())?;
                let n = args[2].parse::<usize>().map_err(|e| e.to_owned())?;
                Ok(Command::ReadLines {
                    file,
                    start,
                    n,
                })
            }
            "JUMP" => {
                if args.len() != 4 {
                    return Err(format!("Expected 4 arguments, got {}", args.len()).into());
                }
                let file = args[0].clone();
                let line = args[1].parse::<usize>().map_err(|e| e.to_string())?;
                let char = args[2].parse::<usize>().map_err(|e| e.to_string())?;
                let n = args[3].parse::<usize>().map_err(|e| e.to_string())?;
                Ok(Command::Jump {
                    file,
                    line,
                    char,
                    n,
                })
            }
            _ => Err(format!("Unknown command: {}", name).into()),
        }
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Command::SearchFiles { keywords } => {
                write!(
                    f,
                    "SearchFiles: query={}",
                    keywords
                        .iter()
                        .map(|v| format!(r#""{}""#, v))
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            }
            Command::ReadLines { file, start, n } => {
                write!(f, "ReadLines: file={}, start={}, n={}", file, start, n)
            }
            Command::Jump { file, line, char, n } => {
                write!(f, "Jump: file={}, line={}, char={}, n={}", file, line, char, n)
            }
        }
    }
}

#[derive(Clone)]
pub enum InjectedMessage {
    AskToSummarize,
}

impl InjectedMessage {
    pub fn get_string(&self) -> String {
        match self {
            InjectedMessage::AskToSummarize => consts::ASK_TO_SUMMARIZE.trim().to_owned(),
        }
    }
}
