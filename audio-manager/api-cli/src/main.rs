use itertools::Itertools;
use reqwest::Client;
use std::{fmt::Display, time::Duration};

use audio_manager_api::{
    audio::{audio_item::AudioMetaData, audio_player::LoopBounds},
    commands::node_commands::{
        AddQueueItemParams, AudioNodeCommand, LoopQueueParams, MoveQueueItemParams,
        PlaySelectedParams, RemoveQueueItemParams, SetAudioProgressParams,
    },
    streams::{brain_streams::AudioBrainInfoStreamType, node_streams::AudioNodeInfoStreamType},
};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
pub struct CliArgs {
    #[command(subcommand)]
    pub action: Action,
    #[arg(short, long, default_value_t = String::from("127.0.0.1"))]
    /// IP address to connect to
    pub addr: String,
    #[arg(short, long, default_value_t = 50051)]
    /// Port to connect to
    pub port: u16,
    #[arg(short, long)]
    /// Only print URL and body instead of performing network actions
    pub dry_run: bool,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Action {
    #[command(about = "Send a command")]
    Send {
        #[command(subcommand)]
        con_type: SendConnectionType,
    },
    #[command(about = "Listen for data")]
    Listen {
        #[command(subcommand)]
        con_type: ListenConnectionType,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum ListenConnectionType {
    #[command(about = "Listen for information from an audio device")]
    Node {
        #[arg(short, long)]
        /// Name of the node to connect to
        source_name: String,
        #[arg(short, long, value_delimiter = ',')]
        /// List of information to listen for
        wanted_info: Vec<AudioNodeInfoStreamType>,
    },
    #[command(about = "Listen for information from the master server")]
    Brain {
        #[arg(short, long, value_delimiter = ',')]
        /// List of information to listen for
        wanted_info: Vec<AudioBrainInfoStreamType>,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum SendConnectionType {
    #[command(about = "Send a command to an udio device")]
    Node {
        #[arg(short, long)]
        /// Name of the node to connect to
        source_name: String,
        #[command(subcommand)]
        cmd: CliNodeCommand,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum CliNodeCommand {
    AddQueueItem {
        #[arg(short, long)]
        url: String,
        #[arg(short, long)]
        name: String,
        #[arg(short, long)]
        author: Option<String>,
        #[arg(short, long, value_parser = parse_duration)]
        duration: Option<Duration>,
        #[arg(short, long)]
        thumbnail_url: Option<String>,
    },
    RemoveQueueItem {
        index: usize,
    },
    MoveQueueItem {
        #[arg(short, long)]
        old_pos: usize,
        #[arg(short, long)]
        new_pos: usize,
    },
    SetAudioProgress {
        #[arg(short, long)]
        progress: f64,
    },
    PauseQueue,
    UnPauseQueue,
    PlayNext,
    PlayPrevious,
    PlaySelected {
        #[arg(short, long)]
        index: usize,
    },
    LoopQueue {
        #[arg(short, long)]
        start: Option<usize>,
        #[arg(short, long)]
        end: Option<usize>,
    },
}

impl Display for ListenConnectionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            Self::Brain { wanted_info } => format!(
                "brain?wanted_info={info}",
                info = wanted_info
                    .iter()
                    .map(|i| serde_json::to_string(i).unwrap())
                    .collect_vec()
                    .join(",")
            ),
            Self::Node {
                source_name,
                wanted_info,
            } => format!(
                "node/{source_name}?wanted_info={info}",
                info = wanted_info
                    .iter()
                    .map(|i| serde_json::to_string(i).unwrap())
                    .collect_vec()
                    .join(",")
            ),
        };

        write!(f, "{str}")
    }
}

impl Display for SendConnectionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            Self::Node { source_name, .. } => format!("commands/{source_name}"),
        };

        write!(f, "{str}")
    }
}

impl Action {
    fn get_prefix_and_endpoint(&self) -> (&str, &str) {
        match self {
            Self::Send { .. } => ("http", "commands"),
            Self::Listen { .. } => ("ws", "streams"),
        }
    }

    fn get_con_type_endpoint(&self) -> String {
        match self {
            Self::Listen { con_type } => format!("{con_type}"),
            Self::Send { con_type } => format!("{con_type}"),
        }
    }

    fn get_body(&self) -> Option<String> {
        match self {
            Self::Send { con_type } => match con_type {
                SendConnectionType::Node { cmd, .. } => {
                    let audio_node_cmd: AudioNodeCommand = cmd.clone().into();
                    let json_str = serde_json::to_string(&audio_node_cmd).unwrap();

                    Some(json_str)
                }
            },
            Self::Listen { .. } => None,
        }
    }
}

impl From<CliNodeCommand> for AudioNodeCommand {
    fn from(value: CliNodeCommand) -> Self {
        match value {
            CliNodeCommand::AddQueueItem {
                url,
                name,
                author,
                duration,
                thumbnail_url,
            } => AudioNodeCommand::AddQueueItem(AddQueueItemParams {
                metadata: AudioMetaData {
                    name,
                    author,
                    duration,
                    thumbnail_url,
                },
                url,
            }),
            CliNodeCommand::RemoveQueueItem { index } => {
                AudioNodeCommand::RemoveQueueItem(RemoveQueueItemParams { index })
            }
            CliNodeCommand::MoveQueueItem { old_pos, new_pos } => {
                AudioNodeCommand::MoveQueueItem(MoveQueueItemParams { old_pos, new_pos })
            }
            CliNodeCommand::SetAudioProgress { progress } => {
                AudioNodeCommand::SetAudioProgress(SetAudioProgressParams { progress })
            }
            CliNodeCommand::PauseQueue => AudioNodeCommand::PauseQueue,
            CliNodeCommand::UnPauseQueue => AudioNodeCommand::UnPauseQueue,
            CliNodeCommand::PlayNext => AudioNodeCommand::PlayNext,
            CliNodeCommand::PlayPrevious => AudioNodeCommand::PlayPrevious,
            CliNodeCommand::PlaySelected { index } => {
                AudioNodeCommand::PlaySelected(PlaySelectedParams { index })
            }
            CliNodeCommand::LoopQueue { start, end } => {
                if start.is_some() && end.is_some() {
                    AudioNodeCommand::LoopQueue(LoopQueueParams {
                        bounds: Some(LoopBounds {
                            start: start.unwrap(),
                            end: end.unwrap(),
                        }),
                    })
                } else {
                    AudioNodeCommand::LoopQueue(LoopQueueParams { bounds: None })
                }
            }
        }
    }
}

fn parse_duration(arg: &str) -> Result<Duration, std::num::ParseIntError> {
    let seconds = arg.parse()?;
    Ok(std::time::Duration::from_secs(seconds))
}

fn get_url_and_body(action: &Action, addr: String, port: u16) -> (String, String) {
    let (prefix, action_endpoint) = action.get_prefix_and_endpoint();
    let con_endpoint = action.get_con_type_endpoint();

    let body = action.get_body().unwrap_or_default();
    let addr = format!(
        "{prefix}://{addr}:{port}/{action_endpoint}/{con_endpoint}",
        addr = addr,
        port = port,
    );

    (addr, body)
}

async fn send_command(url: &str, body: String) -> Result<String, reqwest::Error> {
    let client = Client::new();
    let res = client.post(url).body(body).send().await?;

    Ok(res.text().await?)
}

#[tokio::main]
async fn main() -> Result<(), &'static str> {
    let args = CliArgs::parse();

    let (url, body) = get_url_and_body(&args.action, args.addr, args.port);

    if args.dry_run {
        println!("{url}");
        println!("{body}");
    } else {
        match args.action {
            Action::Send { .. } => {
                let out = send_command(&url, body).await.unwrap();
                println!("{out}");
            }
            Action::Listen { .. } => todo!("help"),
        }
    }

    Ok(())
}
