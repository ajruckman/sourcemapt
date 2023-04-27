use std::fmt;
use openai_dive::v1::resources::chat_completion::{ChatMessage, Role};
use crate::types::{CodeBlock, Command, InjectedMessage};

pub enum SourcemaptMessage {
    System { content: String, hidden: bool },
    User { content: String, hidden: bool },
    Code { code: CodeBlock, hidden: bool },
    Injected { kind: InjectedMessage, hidden: bool },
    Model { content: String, hidden: bool },
    CommandInvocation { command: Command, hidden: bool },
    CommandResult { content: String, hidden: bool },
}

impl SourcemaptMessage {
    pub fn hidden(&self) -> bool {
        match self {
            SourcemaptMessage::System { hidden, .. } => *hidden,
            SourcemaptMessage::User { hidden, .. } => *hidden,
            SourcemaptMessage::Code { hidden, .. } => *hidden,
            SourcemaptMessage::Injected { hidden, .. } => *hidden,
            SourcemaptMessage::Model { hidden, .. } => *hidden,
            SourcemaptMessage::CommandInvocation { hidden, .. } => *hidden,
            SourcemaptMessage::CommandResult { hidden, .. } => *hidden,
        }
    }

    pub fn is_summary(&self) -> bool {
        match self {
            SourcemaptMessage::System { .. } => {}
            SourcemaptMessage::User { .. } => {}
            SourcemaptMessage::Code { .. } => {}
            SourcemaptMessage::Injected { .. } => {}
            SourcemaptMessage::Model { content, .. } => return content.contains("IN SUMMARY:"),
            SourcemaptMessage::CommandInvocation { .. } => {}
            SourcemaptMessage::CommandResult { .. } => {}
        }
        false
    }

    pub fn hide(&mut self) {
        match self {
            SourcemaptMessage::System { hidden, .. } => *hidden = true,
            SourcemaptMessage::User { hidden, .. } => *hidden = true,
            SourcemaptMessage::Code { hidden, .. } => *hidden = true,
            SourcemaptMessage::Injected { hidden, .. } => *hidden = true,
            SourcemaptMessage::Model { hidden, .. } => *hidden = true,
            SourcemaptMessage::CommandInvocation { hidden, .. } => *hidden = true,
            SourcemaptMessage::CommandResult { hidden, .. } => *hidden = true,
        };
    }

    pub fn map_to_chat_message(&self) -> ChatMessage {
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
            SourcemaptMessage::Code { code, .. } => ChatMessage {
                role: Role::User,
                content: code.format(),
                name: None,
            },
            SourcemaptMessage::Injected { kind, .. } => ChatMessage {
                role: Role::User,
                content: kind.get_string(),
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
}

impl fmt::Display for SourcemaptMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (label, content, hidden) = match self {
            SourcemaptMessage::System { content, hidden } => ("System", content.clone(), hidden),
            SourcemaptMessage::User { content, hidden } => ("User", content.clone(), hidden),
            SourcemaptMessage::Code { code, hidden } => ("Code", code.format(), hidden),
            SourcemaptMessage::Injected { kind, hidden } => ("UserInjected", kind.get_string(), hidden),
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
            SourcemaptMessage::Code { code, hidden } => SourcemaptMessage::Code {
                code: code.clone(),
                hidden: *hidden,
            },
            SourcemaptMessage::Injected { kind, hidden } => SourcemaptMessage::Injected {
                kind: kind.clone(),
                hidden: *hidden,
            },
            SourcemaptMessage::Model { content, hidden } => SourcemaptMessage::Model {
                content: content.clone(),
                hidden: *hidden,
            },
            SourcemaptMessage::CommandInvocation { command, hidden } =>
                SourcemaptMessage::CommandInvocation {
                    command: command.clone(),
                    hidden: *hidden,
                },
            SourcemaptMessage::CommandResult { content, hidden } =>
                SourcemaptMessage::CommandResult {
                    content: content.clone(),
                    hidden: *hidden,
                },
        }
    }
}
