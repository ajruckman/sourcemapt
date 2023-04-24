mod consts;

#[macro_use]
mod util;

mod sourcegraph;

use crate::sourcegraph::search_files;
use crossterm::queue;
use openai_dive::v1::api::Client;
use openai_dive::v1::resources::chat_completion::{ChatCompletionParameters, ChatMessage, Role};
use regex::Regex;
use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::str::FromStr;
use std::{fmt, fs, process};
use toml::Value;

#[tokio::main]
async fn main() {
    let api_key = read_or_create_config().unwrap();

    let mut sourcemapt = Sourcemapt::new(api_key, "github.com/kubernetes/kubernetes".to_owned());
    sourcemapt.add_system();

    match sourcemapt.run_loop().await {
        None => {}
        Some(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    }

    for message in sourcemapt.messages {
        println!("{}", message);
    }
}

enum SourcemaptMessage {
    System { content: String, hidden: bool },
    User { content: String, hidden: bool },
    Injected { content: String, hidden: bool },
    Model { content: String, hidden: bool },
    CommandInvocation { command: Command, hidden: bool },
    CommandResult { content: String, hidden: bool },
}

impl SourcemaptMessage {
    fn hidden(&self) -> bool {
        match self {
            SourcemaptMessage::System { hidden, .. } => *hidden,
            SourcemaptMessage::User { hidden, .. } => *hidden,
            SourcemaptMessage::Injected { hidden, .. } => *hidden,
            SourcemaptMessage::Model { hidden, .. } => *hidden,
            SourcemaptMessage::CommandInvocation { hidden, .. } => *hidden,
            SourcemaptMessage::CommandResult { hidden, .. } => *hidden,
        }
    }
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
            SourcemaptMessage::Injected { content, hidden } => SourcemaptMessage::Injected {
                content: content.clone(),
                hidden: *hidden,
            },
            SourcemaptMessage::Model { content, hidden } => SourcemaptMessage::Model {
                content: content.clone(),
                hidden: *hidden,
            },
            SourcemaptMessage::CommandInvocation { command, hidden } => {
                SourcemaptMessage::CommandInvocation {
                    command: command.clone(),
                    hidden: *hidden,
                }
            }
            SourcemaptMessage::CommandResult { content, hidden } => {
                SourcemaptMessage::CommandResult {
                    content: content.clone(),
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
    fn map_to_chat_message(&self) -> ChatMessage {
        match self {
            SourcemaptMessage::System { content, .. } => ChatMessage {
                role: Role::System,
                content: content.clone(),
                name: None,
            },
            SourcemaptMessage::User { content, .. } => ChatMessage {
                role: Role::User,
                content: content.clone(),
                name: None,
            },
            SourcemaptMessage::Injected { content, .. } => ChatMessage {
                role: Role::User,
                content: content.clone(),
                name: None,
            },
            SourcemaptMessage::Model { content, .. } => ChatMessage {
                role: Role::Assistant,
                content: content.clone(),
                name: None,
            },
            SourcemaptMessage::CommandInvocation { command, .. } => ChatMessage {
                role: Role::Assistant,
                content: command.serialize(),
                name: None,
            },
            SourcemaptMessage::CommandResult { content, .. } => ChatMessage {
                role: Role::User,
                content: content.clone(),
                name: None,
            },
        }
    }

    fn map_to_role(&self) -> Role {
        match self {
            SourcemaptMessage::System { .. } => Role::System,
            SourcemaptMessage::User { .. } => Role::User,
            SourcemaptMessage::Injected { .. } => Role::User,
            SourcemaptMessage::Model { .. } => Role::Assistant,
            SourcemaptMessage::CommandInvocation { .. } => Role::Assistant,
            SourcemaptMessage::CommandResult { .. } => Role::User,
        }
    }
}

impl fmt::Display for SourcemaptMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (label, content, hidden) = match self {
            SourcemaptMessage::System { content, hidden } => ("System", content.clone(), hidden),
            SourcemaptMessage::User { content, hidden } => ("User", content.clone(), hidden),
            SourcemaptMessage::Injected { content, hidden } => ("UserInjected", content.clone(), hidden),
            SourcemaptMessage::Model { content, hidden } => ("ModelMessage", content.clone(), hidden),
            SourcemaptMessage::CommandInvocation { command, hidden } => {
                ("CommandInvocation", format!("{}", command), hidden)
            }
            SourcemaptMessage::CommandResult { content, hidden } => ("CommandResult", content.clone(), hidden),
        };

        write!(f, "{} [hidden: {}]\n", label, hidden)?;
        write_lines(f, &content)
    }
}

fn write_lines(f: &mut fmt::Formatter<'_>, content: &str) -> fmt::Result {
    for line in content.lines() {
        writeln!(f, "| {}", line)?;
    }
    Ok(())
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

    messages: Vec<SourcemaptMessage>,

    repo: String,
}

enum ProcessResponsesOutcome {
    CallForIntrospect,
    CallWithCommandResults(Vec<SourcemaptMessage>),
    Stop,
}

impl Sourcemapt {
    fn new(api_key: String, repo: String) -> Self {
        Self {
            client: Client::new(api_key),
            messages: Vec::new(),
            repo: repo,
        }
    }

    fn add_system(&mut self) {
        self.messages.push(SourcemaptMessage::System {
            content: consts::SYSTEM.to_string(),
            hidden: false,
        });
    }

    async fn run_loop(&mut self) -> Option<Box<dyn Error>> {
        let mut responses;

        responses = self.call_gpt4(
            &vec![SourcemaptMessage::User {
                content: "I'm interested to know how the kubelet volume manager determines whether reconciler states have been synced. What is some relevant code?".to_string(),
                hidden: false,
            }],
        ).await.ok()?.to_vec();

        loop {
            for response in &responses {
                println!("{}", response);
            }

            let result = self.process_responses(&responses).await.ok()?;

            println!();

            match result {
                ProcessResponsesOutcome::CallForIntrospect => {
                    print_success!("-> Outcome: Introspect");
                    // TODO: Remove
                    // responses = self.call_gpt4(&Vec::new()).await.unwrap().to_vec();

                    // responses = self.call_gpt4(&vec![
                    //     SourcemaptMessage::Injected {
                    //         content: "Can I see some more content from the first file?".to_string(),
                    //         hidden: false,
                    //     }
                    // ]).await.unwrap().to_vec();

                    responses = self.call_gpt4(&vec![
                        SourcemaptMessage::Injected {
                            content: consts::ASK_TO_SUMMARIZE.to_owned(),
                            hidden: false,
                        }
                    ]).await.unwrap().to_vec();
                }
                ProcessResponsesOutcome::CallWithCommandResults(results) => {
                    print_success!("-> Outcome: Call with command results:");
                    for result in &results {
                        print_success!("   {}", result);
                    }
                    responses = self.call_gpt4(&results).await.unwrap().to_vec();
                }
                ProcessResponsesOutcome::Stop => {
                    print_success!("-> Outcome: Stop");
                    return None;
                }
            }
        }
    }

    async fn call_gpt4(
        &mut self,
        messages: &[SourcemaptMessage],
    ) -> Result<&[SourcemaptMessage], Box<dyn Error>> {
        for message in messages {
            self.messages.push(message.clone());
        }

        let mut prompt_messages = Vec::new();

        for hist_message in &self.messages {
            if hist_message.hidden() { continue; }
            prompt_messages.push(hist_message.map_to_chat_message());
        }

        let hist_end = self.messages.len();

        //

        let parameters = ChatCompletionParameters {
            model: "gpt-4".to_string(),
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

        let completion_r = self.client.chat().create(parameters).await.unwrap();
        let completion = completion_r.choices[0].message.content.trim();

        // println!("-----");
        // println!("{}", completion);
        // println!("-----");

        let mut buffer = String::new();
        let mut skip_next_line = false;

        for line in completion.lines() {
            if skip_next_line {
                skip_next_line = false;
                continue;
            }

            if Command::match_line(line) {
                if !buffer.trim().is_empty() && buffer.trim() != "```" && buffer.trim() != r#"""""# {
                    self.messages.push(SourcemaptMessage::Model {
                        content: buffer.trim().to_owned(),
                        hidden: false,
                    });
                    buffer.clear();
                }

                let command = line.parse::<Command>()?;
                self.messages.push(SourcemaptMessage::CommandInvocation {
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
            self.messages.push(SourcemaptMessage::Model {
                content: buffer.trim().to_owned(),
                hidden: false,
            });
        }

        Ok(&self.messages[hist_end..])
    }

    async fn process_responses(
        &mut self,
        responses: &[SourcemaptMessage],
    ) -> Result<ProcessResponsesOutcome, Box<dyn Error>> {
        let mut command_results = Vec::new();

        for response in responses {
            match response {
                SourcemaptMessage::Model { .. } => {}
                SourcemaptMessage::CommandInvocation { command, hidden } => {
                    println!("");
                    match command {
                        Command::SearchFiles { keywords } => {
                            let res = search_files::search_files(
                                &self.repo,
                                keywords.as_slice(),
                            ).await?;

                            let json = serde_json::to_string(&res)?;

                            command_results.push(SourcemaptMessage::CommandResult {
                                content: json,
                                hidden: false,
                            })
                        }
                        Command::ReadLines { file, start, n } => {
                            let content = get_file_content(&self.repo, file).await.unwrap();

                            let lines = content.lines()
                                .skip(*start)
                                .take(*n)
                                .collect::<Vec<&str>>()
                                .join("\n");

                            command_results.push(SourcemaptMessage::CommandResult {
                                content: lines,
                                hidden: false,
                            });
                        }
                    }
                }
                _ => {
                    eprintln!("Unexpected response message: {}", response);
                }
            }
        }

        if command_results.is_empty() {
            return Ok(ProcessResponsesOutcome::CallForIntrospect);
        }

        return Ok(ProcessResponsesOutcome::CallWithCommandResults(command_results));
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
        process::exit(1);
    }

    let config = fs::read_to_string(&config_path)?.parse::<Value>()?;

    let key = match config.get("key") {
        Some(key) => key.as_str().unwrap_or("").to_string(),
        None => {
            print_error!(
                "The 'key' value is not set in the configuration file: {}",
                config_path.display()
            );
            process::exit(1);
        }
    };

    if key.is_empty() {
        print_error!(
            "Set the 'key' value in the configuration file before using the program: {}",
            config_path.display()
        );
        process::exit(1);
    }

    Ok(key)
}

async fn get_file_content(repo_url: &str, file_path: &str) -> Result<String, Box<dyn Error>> {
    let (host, repo) = parse_repo_url(repo_url).unwrap();
    let api_url = match host.as_str() {
        "github.com" => format!(
            "https://api.github.com/repos/{}/contents/{}",
            repo, file_path
        ),
        "gitlab.com" => format!(
            "https://gitlab.com/api/v4/projects/{}/repository/files/{}/raw",
            repo.replace("/", "%2F"),
            file_path.replace("/", "%2F")
        ),
        _ => return Err("unsupported host".into()),
    };

    let client = reqwest::Client::builder()
        .user_agent("dev.ruckman.sourcemapt")
        .build()?;
    let resp = client.get(&api_url).send().await.unwrap();

    let resp = resp.json::<Value>().await.unwrap();

    let content_base64 = match host.as_str() {
        "github.com" => {
            let content = resp["content"].as_str().ok_or("content not found")?;
            content.replace("\n", "")
        }
        "gitlab.com" => {
            let content = resp["content"].as_str().ok_or("content not found").unwrap();
            base64::encode(content)
        }
        _ => return Err("unsupported host".into()),
    };

    let content = base64::decode(content_base64).unwrap().into_iter().map(|c| c as char).collect();

    Ok(content)
}

fn parse_repo_url(repo_url: &str) -> Result<(String, String), Box<dyn Error>> {
    let parts: Vec<&str> = repo_url.split('/').collect();
    if parts.len() != 3 {
        return Err("invalid repo url".into());
    }

    let host = parts[0].to_string();
    let repo = format!("{}/{}", parts[1], parts[2]);

    Ok((host, repo))
}
