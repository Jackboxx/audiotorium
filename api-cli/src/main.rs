use itertools::Itertools;
use reqwest::Client;
use std::{
    fmt::Display,
    process::{Command, Stdio},
};
use websocket::{ClientBuilder, OwnedMessage};

use audio_manager_api::{
    audio_playback::audio_player::LoopBounds,
    commands::node_commands::{
        AddQueueItemParams, AudioNodeCommand, DownloadIdentifierParam, LoopQueueParams,
        MoveQueueItemParams, PlaySelectedParams, RemoveQueueItemParams, SetAudioProgressParams,
        SetAudioVolumeParams,
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
        #[arg(short, long)]
        // command to run on received messages. None = print to stdout
        command: Option<String>,
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
    SetAudioVolume {
        #[arg(short, long)]
        volume: f32,
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
                    .map(|i| serde_json::to_string(i).unwrap().replace('"', ""))
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
                    .map(|i| serde_json::to_string(i).unwrap().replace('"', ""))
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
            Self::Node { source_name, .. } => format!("node/{source_name}"),
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
            Self::Listen { con_type, .. } => format!("{con_type}"),
            Self::Send { con_type } => format!("{con_type}"),
        }
    }
}

impl From<CliNodeCommand> for AudioNodeCommand {
    fn from(value: CliNodeCommand) -> Self {
        match value {
            CliNodeCommand::AddQueueItem { url } => {
                AudioNodeCommand::AddQueueItem(AddQueueItemParams {
                    identifier: DownloadIdentifierParam::YouTube { url },
                })
            }
            CliNodeCommand::RemoveQueueItem { index } => {
                AudioNodeCommand::RemoveQueueItem(RemoveQueueItemParams { index })
            }
            CliNodeCommand::MoveQueueItem { old_pos, new_pos } => {
                AudioNodeCommand::MoveQueueItem(MoveQueueItemParams { old_pos, new_pos })
            }
            CliNodeCommand::SetAudioVolume { volume } => {
                AudioNodeCommand::SetAudioVolume(SetAudioVolumeParams { volume })
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

fn get_url(action: &Action, addr: String, port: u16) -> String {
    let (prefix, action_endpoint) = action.get_prefix_and_endpoint();
    let con_endpoint = action.get_con_type_endpoint();

    format!(
        "{prefix}://{addr}:{port}/{action_endpoint}/{con_endpoint}",
        addr = addr,
        port = port,
    )
}

fn get_body(action: &Action) -> Option<AudioNodeCommand> {
    match action {
        Action::Send { con_type } => match con_type {
            SendConnectionType::Node { cmd, .. } => Some(cmd.clone().into()),
        },
        _ => None,
    }
}

async fn send_command(url: &str, body: &AudioNodeCommand) -> Result<String, reqwest::Error> {
    let client = Client::new();
    let res = client.post(url).json(body).send().await?;

    Ok(res.text().await?)
}

fn listen_on_socket(url: &str, cmd_str: Option<String>) {
    let client = ClientBuilder::new(url)
        .unwrap()
        .add_protocol("rust-websocket")
        .connect_insecure()
        .unwrap();

    let (mut receiver, _) = client.split().unwrap();

    for message in receiver.incoming_messages() {
        match message {
            Ok(OwnedMessage::Text(text)) => match cmd_str {
                Some(ref cmd_str) => {
                    let (cmd, args) = cmd_str.split_once(" ").unwrap_or((cmd_str, ""));

                    let echo_cmd = Command::new("echo")
                        .arg(text)
                        .stdout(Stdio::piped())
                        .spawn()
                        .unwrap();

                    let cmd = Command::new(cmd)
                        .arg(args)
                        .stdin(Stdio::from(echo_cmd.stdout.unwrap()))
                        .spawn()
                        .unwrap();

                    let out = cmd.wait_with_output().expect("Failed to read stdout");

                    println!("{}", String::from_utf8_lossy(&out.stdout).to_string())
                }
                None => {
                    println!("{text}");
                }
            },
            _ => {}
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), &'static str> {
    let args = CliArgs::parse();

    let url = get_url(&args.action, args.addr, args.port);
    let body = get_body(&args.action);

    if args.dry_run {
        let str_body = body
            .map(|b| serde_json::to_string(&b).unwrap())
            .unwrap_or_default();

        println!("{url}");
        println!("{str_body}");
    } else {
        match args.action {
            Action::Send { .. } => {
                let out = send_command(&url, body.as_ref().unwrap()).await.unwrap();
                println!("{out}");
            }
            Action::Listen { command, .. } => {
                listen_on_socket(&url, command);
            }
        }
    }

    Ok(())
}
