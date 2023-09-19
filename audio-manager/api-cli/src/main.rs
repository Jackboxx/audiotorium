use itertools::Itertools;
use std::fmt::Display;

use audio_manager_api::streams::{
    brain_streams::AudioBrainInfoStreamType, node_streams::AudioNodeInfoStreamType,
};
use clap::{Parser, Subcommand};

#[derive(Parser)]
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
}

#[derive(Subcommand)]
pub enum Action {
    #[command(about = "TODO")]
    Send {
        #[command(subcommand)]
        con_type: SendConnectionType,
    },
    #[command(about = "TODO")]
    Listen {
        #[command(subcommand)]
        con_type: ListenConnectionType,
    },
}

#[derive(Subcommand)]
pub enum ListenConnectionType {
    #[command(about = "TODO")]
    Node {
        #[arg(short, long)]
        /// Name of the node to connect to
        source_name: String,
        #[arg(short, long, value_delimiter = ',')]
        /// List of information to listen for
        wanted_info: Vec<AudioNodeInfoStreamType>,
    },
    #[command(about = "TODO")]
    Brain {
        #[arg(short, long, value_delimiter = ',')]
        /// List of information to listen for
        wanted_info: Vec<AudioBrainInfoStreamType>,
    },
}

#[derive(Subcommand)]
pub enum SendConnectionType {
    #[command(about = "TODO")]
    Node {
        #[arg(short, long)]
        /// Name of the node to connect to
        source_name: String,
        #[arg(short, long)]
        /// Command to send to the node
        cmd: String,
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
}

fn main() -> Result<(), &'static str> {
    let args = CliArgs::parse();

    let (prefix, action_endpoint) = args.action.get_prefix_and_endpoint();
    let con_endpoint = args.action.get_con_type_endpoint();

    let addr = format!(
        "{prefix}://{addr}:{port}/{action_endpoint}/{con_endpoint}",
        addr = args.addr,
        port = args.port,
    );

    println!("{addr}");

    Ok(())
}
