extern crate core;

mod consts;

#[macro_use]
mod util;

mod message;
mod sourcegraph;
mod types;

use crate::message::SourcemaptMessage;
use crate::sourcegraph::client::SourcegraphClient;
use crate::types::{CodeBlock, Command, InjectedMessage};
use crossterm::queue;
use openai_dive::v1::api::Client;
use openai_dive::v1::resources::chat_completion::ChatCompletionParameters;
use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::{fs, process};
use toml::Value;

// TODO: Side analyzer to strip licenses, irrelevant comments, etc. from GET_LINES to save tokens

// TODO: "Decision" system which keeps track of a set of shown data which can all be hidden when
// the AI makes a decision
// For example, let the AI traverse the directory tree to find files it might want to look at
// These messages don't need to be kept once the AI decides what file it wants to see

#[tokio::main]
async fn main() {
    let api_key = read_or_create_config().unwrap();

    let mut sourcemapt = Sourcemapt::new(
        api_key,
        "github.com/kubernetes/kubernetes".to_owned(),
        "master".to_owned(),
    );
    sourcemapt.add_system();

    let def = sourcemapt.sourcegraph_client.get_definition(
        "github.com/kubernetes/kubezrnetes",
        "master",
        "test/e2e/framework/pod/wait.go",
        433,
        24,
    ).await.unwrap_or_default();

    match sourcemapt.run_loop().await {
        None => {}
        Some(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    }

    let _ = def;

    for message in sourcemapt.messages {
        println!("{}", message);
    }
}

struct Sourcemapt {
    openai_client: Client,
    sourcegraph_client: SourcegraphClient,

    messages: Vec<SourcemaptMessage>,

    repo: String,
    refspec: String,
}

enum ProcessResponsesOutcome {
    CallForIntrospect,
    CallWithCommandResults(Vec<SourcemaptMessage>),
    Stop,
}

impl Sourcemapt {
    fn new(api_key: String, repo: String, refspec: String) -> Self {
        Self {
            openai_client: Client::new(api_key),
            sourcegraph_client: SourcegraphClient::new(),
            messages: Vec::new(),
            repo: repo,
            refspec: refspec,
        }
    }

    fn add_system(&mut self) {
        self.messages.push(SourcemaptMessage::System {
            content: consts::SYSTEM.trim().to_owned(),
            hidden: false,
        });
    }

    async fn run_loop(&mut self) -> Option<Box<dyn Error>> {
        let mut responses;

        responses = self.call_gpt4(
            &vec![SourcemaptMessage::User {
                // content: "I'm interested to know how the kubelet volume manager determines whether reconciler states have been synced. What is some relevant code?".to_owned(),
                content: "What is the purpose of the `endpoints.RepackSubsets(subsets)` function call in the Endpoints Controller syncService, and how does it affect the resulting `subsets`?".to_owned(),
                hidden: false,
            }],
        ).await.ok()?.to_vec();

        loop {
            for response in &responses {
                println!("{}", response);
            }

            let result = self.process_responses(&responses).await.ok()?;

            match result {
                ProcessResponsesOutcome::CallForIntrospect => {
                    print_success!("-> Outcome: Introspect");
                    // TODO: Remove
                    // responses = self.call_gpt4(&Vec::new()).await.unwrap().to_vec();

                    // responses = self.call_gpt4(&vec![
                    //     SourcemaptMessage::Injected {
                    //         content: "Can I see some more content from the first file?".to_owned(),
                    //         hidden: false,
                    //     }
                    // ]).await.unwrap().to_vec();

                    responses = self.call_gpt4(&vec![
                        SourcemaptMessage::Injected {
                            kind: InjectedMessage::AskToSummarize,
                            hidden: false,
                        }
                    ]).await.unwrap().to_vec();
                }
                ProcessResponsesOutcome::CallWithCommandResults(results) => {
                    print_success!("-> Outcome: Call with command results:");
                    for result in &results {
                        print_success!("| {}", result);
                    }
                    responses = self.call_gpt4(&results).await.unwrap().to_vec();
                }
                ProcessResponsesOutcome::Stop => {
                    print_success!("-> Outcome: Stop");
                    return None;
                }
            }

            self.compact();
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
            model: "gpt-4".to_owned(),
            messages: prompt_messages,
            temperature: None,
            top_p: Some(0.1),
            n: None,
            stop: None,
            max_tokens: Some(512),
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
        };

        let completion_r = self.openai_client.chat().create(parameters).await.unwrap();
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
            println!("");

            match response {
                SourcemaptMessage::Model { .. } => {}
                SourcemaptMessage::CommandInvocation { command, .. } => {
                    match command {
                        Command::SearchFiles { keywords } => {
                            let res = self.sourcegraph_client.search_files(
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
                            let content = self.sourcegraph_client.get_file_content(
                                &self.repo,
                                &self.refspec,
                                file,
                            ).await?.content;

                            let lines = content.lines()
                                .skip(*start)
                                .take(*n)
                                .map(|v| v.to_owned())
                                .collect::<Vec<String>>();

                            command_results.push(SourcemaptMessage::Code {
                                code: CodeBlock {
                                    lines: lines,
                                    start: *start,
                                },
                                hidden: false,
                            });
                        }
                        Command::Jump { file, line, char, n } => {
                            let definition_result = self.sourcegraph_client.get_definition(
                                &self.repo,
                                &self.refspec,
                                file,
                                *line as u32,
                                *char as u32,
                            ).await?;

                            match definition_result {
                                None => {
                                    command_results.push(SourcemaptMessage::User {
                                        content: format!("Couldn't find definition for `{}`", file),
                                        hidden: false,
                                    });
                                }
                                Some(v) => {
                                    let def = v.definitions.first().unwrap();

                                    let content = self.sourcegraph_client.get_file_content(
                                        &def.resource.repo,
                                        &def.resource.commit_oid,
                                        &def.resource.path,
                                    ).await?.content;

                                    let lines = content.lines()
                                        .skip(def.range.line_start as usize)
                                        .take(*n)
                                        .map(|v| v.to_owned())
                                        .collect::<Vec<String>>();

                                    command_results.push(SourcemaptMessage::Code {
                                        code: CodeBlock {
                                            lines: lines,
                                            start: def.range.line_start as usize,
                                        },
                                        hidden: false,
                                    });
                                }
                            }
                        }
                    }
                }
                _ => {
                    eprintln!("Unexpected response message: {}", response);
                }
            }
        }

        if !(command_results.is_empty()) {
            return Ok(ProcessResponsesOutcome::CallWithCommandResults(command_results));
        }

        if let Some(last) = responses.last() {
            if let SourcemaptMessage::Model { .. } = last {
                if last.is_summary() {
                    return Ok(ProcessResponsesOutcome::Stop);
                }
            }
        }

        Ok(ProcessResponsesOutcome::CallForIntrospect)
    }

    fn compact(&mut self) {
        let mut messages = self.messages.iter_mut().peekable();

        while let Some(message) = messages.next() {
            if message.hidden() { continue; }
            if let SourcemaptMessage::Injected { kind, .. } = message {
                if let InjectedMessage::AskToSummarize = kind {
                    if let Some(next) = messages.peek() {
                        if !next.is_summary() {
                            print_progress!("Hiding AskToSummarize message");
                            message.hide();
                        }
                    }
                }
            }
        }

        for message in &self.messages {
            match message {
                SourcemaptMessage::System { .. } => {}
                SourcemaptMessage::User { .. } => {}
                SourcemaptMessage::Code { .. } => {}
                SourcemaptMessage::Injected { .. } => {}
                SourcemaptMessage::Model { .. } => {}
                SourcemaptMessage::CommandInvocation { .. } => {}
                SourcemaptMessage::CommandResult { .. } => {}
            }
        }
    }
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
        Some(key) => key.as_str().unwrap_or("").to_owned(),
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
