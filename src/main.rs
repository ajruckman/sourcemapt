mod consts;

#[macro_use]
mod util;

use crossterm::queue;
use futures::stream::iter;
use futures::StreamExt;
use openai_dive::v1::api::Client;
use openai_dive::v1::resources::chat_completion::{ChatCompletionParameters, ChatMessage, Role};
use regex::Regex;
use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::str::FromStr;
use std::{fmt, fs};
use tokio::stream;
use toml::Value;

#[tokio::main]
async fn main() {
    let api_key = read_or_create_config().unwrap();

    let mut sourcemapt = Sourcemapt::new(api_key);

    let responses = sourcemapt.call_gpt4(
        Role::User,
        "I'm interested to know how the kubelet volume manager determines whether reconciler states have been synced. What is some relevant code?".to_string(),
        consts::TEST1,
    ).await.unwrap();

    for response in responses {
        println!("{}", response);
    }

    let responses = sourcemapt
        .call_gpt4(
            Role::User,
            "Can you summarize what the code does?".to_string(),
            consts::TEST2,
        )
        .await
        .unwrap();

    for response in responses {
        println!("{}", response);
    }
}

enum SourcemaptMessage {
    System { content: String, hidden: bool },
    User { content: String, hidden: bool },
    UserInjected { content: String, hidden: bool },
    ModelMessage { content: String, hidden: bool },
    ModelCommand { command: Command, hidden: bool },
}

impl Clone for SourcemaptMessage {
    fn clone(&self) -> Self {
        match self {
            SourcemaptMessage::System { content, hidden } => SourcemaptMessage::System {
                content: content.clone(),
                hidden: *hidden,
            },
            SourcemaptMessage::User { content, hidden } => SourcemaptMessage::User {
                content: content.clone(),
                hidden: *hidden,
            },
            SourcemaptMessage::UserInjected { content, hidden } => {
                SourcemaptMessage::UserInjected {
                    content: content.clone(),
                    hidden: *hidden,
                }
            }
            SourcemaptMessage::ModelMessage { content, hidden } => {
                SourcemaptMessage::ModelMessage {
                    content: content.clone(),
                    hidden: *hidden,
                }
            }
            SourcemaptMessage::ModelCommand { command, hidden } => {
                SourcemaptMessage::ModelCommand {
                    command: command.clone(),
                    hidden: *hidden,
                }
            }
        }
    }
}

#[derive(Debug, PartialEq)]
enum Command {
    SearchFiles {
        keywords: Vec<String>,
    },
    ReadLines {
        file: String,
        start: usize,
        n: usize,
    },
}

impl Command {
    fn match_line(line: &str) -> bool {
        Regex::new(r#"^`?!(\w+) (?:"([^"]+)"*(?: |$|`$))*"#)
            .unwrap()
            .is_match(line)
    }
    fn serialize(&self) -> String {
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
                format!(r#"!READ_LINES "{}" "{}" "{}"#, file, start, n)
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
        }
    }
}

impl FromStr for Command {
    type Err = Box<dyn Error>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // let re = Regex::new(r#"^`?! *(\w+)(?: +"([^"]+)")+ *`?$"#).unwrap();

        let re = Regex::new(r#"^`?!(\w+)((?:\s+"[^"]+")*)"#).unwrap();
        let captures = re.captures(s).expect("Invalid command format");
        let name = captures.get(1).map_or("", |m| m.as_str()).to_string();

        let args_str = captures.get(2).map_or("", |m| m.as_str());
        let re_args = Regex::new(r#""([^"]+)""#).unwrap();
        let args = re_args
            .captures_iter(args_str)
            .map(|c| c.get(1).unwrap().as_str().to_string())
            .collect::<Vec<_>>();

        match name.as_str() {
            "SEARCH_FILES" => Ok(Command::SearchFiles { keywords: args }),
            "READ_LINES" => {
                if args.len() != 3 {
                    return Err(format!("Expected 3 arguments, got {}", args.len()).into());
                }
                let start = args[1].parse::<usize>().map_err(|e| e.to_string())?;
                let n = args[2].parse::<usize>().map_err(|e| e.to_string())?;
                Ok(Command::ReadLines {
                    file: args[0].clone(),
                    start,
                    n,
                })
            }
            _ => Err(format!("Unknown command: {}", name).into()),
        }
    }
}

impl SourcemaptMessage {
    // Returns None if `hidden` is true
    fn map_to_chat_message(&self) -> Option<ChatMessage> {
        match self {
            SourcemaptMessage::System { content, hidden } => {
                if *hidden { return None; }
                Some(ChatMessage {
                    role: Role::System,
                    content: content.clone(),
                    name: None,
                })
            }
            SourcemaptMessage::User { content, hidden } => {
                if *hidden { return None; }
                Some(ChatMessage {
                    role: Role::User,
                    content: content.clone(),
                    name: None,
                })
            }
            SourcemaptMessage::UserInjected { content, hidden } => {
                if *hidden { return None; }
                Some(ChatMessage {
                    role: Role::User,
                    content: content.clone(),
                    name: None,
                })
            }
            SourcemaptMessage::ModelMessage { content, hidden } => {
                if *hidden { return None; }
                Some(ChatMessage {
                    role: Role::Assistant,
                    content: content.clone(),
                    name: None,
                })
            }
            SourcemaptMessage::ModelCommand { command, hidden } => {
                if *hidden { return None; }
                Some(ChatMessage {
                    role: Role::Assistant,
                    content: command.serialize(),
                    name: None,
                })
            }
        }
    }

    fn map_to_role(&self) -> Role {
        match self {
            SourcemaptMessage::System { .. } => Role::System,
            SourcemaptMessage::User { .. } => Role::User,
            SourcemaptMessage::UserInjected { .. } => Role::User,
            SourcemaptMessage::ModelMessage { .. } => Role::Assistant,
            SourcemaptMessage::ModelCommand { .. } => Role::Assistant,
        }
    }
}

impl fmt::Display for SourcemaptMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SourcemaptMessage::System { content, hidden } => {
                write!(f, "System [hidden: {}]: @@@{}@@@", hidden, content)
            }
            SourcemaptMessage::User { content, hidden } => {
                write!(f, "User [hidden: {}]: @@@{}@@@", hidden, content)
            }
            SourcemaptMessage::UserInjected { content, hidden } => {
                write!(f, "UserInjected [hidden: {}]: @@@{}@@@", hidden, content)
            }
            SourcemaptMessage::ModelMessage { content, hidden } => {
                write!(f, "ModelMessage [hidden: {}]: @@@{}@@@", hidden, content)
            }
            SourcemaptMessage::ModelCommand { command, hidden } => {
                write!(f, "ModelCommand [hidden: {}]: @@@{}@@@", hidden, command)
            }
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
        }
    }
}

struct Sourcemapt {
    client: Client,

    history: Vec<SourcemaptMessage>,
}

impl Sourcemapt {
    fn new(api_key: String) -> Self {
        Self {
            client: Client::new(api_key),
            history: Vec::new(),
        }
    }

    async fn call_gpt4(
        &mut self,
        role: Role,
        content: String,
        completion: &str,
    ) -> Result<&[SourcemaptMessage], Box<dyn Error>> {
        let mut prompt_messages = vec![ChatMessage {
            role: Role::System,
            content: consts::SYSTEM.to_string(),
            name: None,
        }];

        for hist_message in &self.history {
            match hist_message.map_to_chat_message() {
                None => {}
                Some(v) => prompt_messages.push(v),
            }
        }

        //

        let hist_end = self.history.len();
        let message = match role {
            Role::User => SourcemaptMessage::User {
                content: content.clone(),
                hidden: false,
            },
            Role::Assistant => SourcemaptMessage::ModelMessage {
                content: content.clone(),
                hidden: false,
            },
            _ => {
                panic!("Invalid call role: {:?}", role)
            }
        };
        self.history.push(message);

        let prompt_message = ChatMessage {
            role: clone_role(&role),
            content: content.clone(),
            name: None,
        };
        prompt_messages.push(prompt_message);

        //

        let parameters = ChatCompletionParameters {
            model: "gpt-3.5-turbo".to_string(),
            messages: prompt_messages,
            temperature: None,
            top_p: None,
            n: None,
            stop: None,
            max_tokens: Some(512),
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
        };

        // let completion_r = self.client.chat().create(parameters).await.unwrap();
        // let completion = completion_r.choices[0].message.content.trim();

        println!("-----");
        println!("{}", completion);
        println!("-----");

        let mut buffer = String::new();
        let mut skip_next_line = false;

        for line in completion.lines() {
            if skip_next_line {
                skip_next_line = false;
                continue;
            }

            if Command::match_line(line) {
                if !buffer.trim().is_empty() && buffer.trim() != "```" && buffer.trim() != r#"""""# {
                    self.history.push(SourcemaptMessage::ModelMessage {
                        content: buffer.trim().to_owned(),
                        hidden: false,
                    });
                    buffer.clear();
                }

                let command = line.parse::<Command>()?;
                self.history.push(SourcemaptMessage::ModelCommand {
                    command,
                    hidden: false,
                });

                let next_line = completion.lines().next();
                if let Some(next_line) = next_line {
                    if next_line.trim() == "```" || next_line.trim() == r#"""""# {
                        skip_next_line = true;
                    }
                }
            } else {
                if line.trim() != "```" && line.trim() != r#"""""# {
                    buffer.push_str(line);
                    buffer.push('\n');
                }
            }
        }

        if !buffer.trim().is_empty() {
            self.history.push(SourcemaptMessage::ModelMessage {
                content: buffer.trim().to_owned(),
                hidden: false,
            });
        }

        Ok(&self.history[hist_end..])
    }

    fn eval_should_hide(&mut self) {}
}

// TODO: If a Model response contains the same code sent as a User message (the model is attempting
// to show the user what it found), cut that code from the response.

fn read_or_create_config() -> Result<String, Box<dyn Error>> {
    let config_dir = dirs::config_dir().ok_or("Unable to find config directory")?;
    let config_path = config_dir.join("sourcemapt.toml");

    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)?;
    }

    if !config_path.exists() {
        let mut file = File::create(&config_path)?;
        file.write_all(br#"key = """#)?;
        print_success!(
            "Created a new configuration file at: {}",
            config_path.display()
        );
        print_success!("Set the 'key' value in the file before using the program.");
        std::process::exit(1);
    }

    let config = fs::read_to_string(&config_path)?.parse::<Value>()?;

    let key = match config.get("key") {
        Some(key) => key.as_str().unwrap_or("").to_string(),
        None => {
            print_error!(
                "The 'key' value is not set in the configuration file: {}",
                config_path.display()
            );
            std::process::exit(1);
        }
    };

    if key.is_empty() {
        print_error!(
            "Set the 'key' value in the configuration file before using the program: {}",
            config_path.display()
        );
        std::process::exit(1);
    }

    Ok(key)
}

// Surely there's a better way to do this lol.
fn clone_role(role: &Role) -> Role {
    match role {
        Role::System => Role::System,
        Role::User => Role::User,
        Role::Assistant => Role::Assistant,
    }
}
