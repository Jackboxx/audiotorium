use std::{env, fs, path::PathBuf};

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
pub struct CliArgs {
    #[command(subcommand)]
    pub action: Action,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Action {
    #[command(about = "Create dummy devices")]
    Create {
        /// Amount of dummy devices to create
        #[arg(short, long, default_value_t = 3)]
        amount: usize,
    },

    #[command(about = "Create cava configs")]
    ConfCava {
        /// Amount of config files to create
        #[arg(short, long, default_value_t = 3)]
        amount: usize,
        #[arg(short, long)]
        out_dir: Option<PathBuf>,
    },
}

fn main() {
    let CliArgs { action } = CliArgs::parse();

    match action {
        Action::Create { amount } => {
            let entries = (0..amount.clamp(1, 8))
                .map(|i| gen_asoundrc_conf(i))
                .collect::<Vec<_>>()
                .join("\n\n");

            let home = env::var("HOME").expect("HOME env var should exits");
            let path = PathBuf::from(home).join(".asoundrc");

            fs::write(path, entries).expect("should be able to write to $HOME/.asoundrc");
        }
        Action::ConfCava { amount, out_dir } => {
            let out_dir = out_dir.unwrap_or(PathBuf::from("cava"));
            fs::create_dir_all(&out_dir).unwrap();

            (0..amount.clamp(1, 8))
                .map(|i| gen_cava_conf(i))
                .enumerate()
                .for_each(|(i, conf)| {
                    let path = out_dir.join(format!("config_{i}"));
                    fs::write(path, conf).unwrap();
                });
        }
    }
}

fn gen_cava_conf(index: usize) -> String {
    format!(
        r#"[input]
method = alsa
source = hw:Loopback,1,{index}
sample_rate = 48000"#
    )
}

fn gen_asoundrc_conf(index: usize) -> String {
    format!(
        r#"pcm.dummy_out_{index} {{
        type plug
        slave {{
                pcm "hw:Loopback,0,{index}"
        }}
        hint {{
                show on
                description "dummy device"
        }}
}}
ctl.dummy_out_{index} {{
        type dmix
}}

pcm.dummy_in_{index} {{
        type plug
        slave {{
                pcm "hw:Loopback,1,{index}"
        }}
        hint {{
                show on
                description "dummy device"
        }}
}}
ctl.dummy_in_{index} {{
        type dmix
}}"#
    )
}
