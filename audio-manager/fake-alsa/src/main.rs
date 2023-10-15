use std::{env, fs, path::PathBuf};

const LOOPBACK_CARD_ID: u8 = 3;

fn main() {
    let entries = (0..3)
        .map(|i| generate_asoundrc_entry(&format!("dummy_{i}"), LOOPBACK_CARD_ID))
        .collect::<Vec<_>>()
        .join("\n");

    let home = env::var("HOME").expect("HOME env var should exits");
    let path = PathBuf::from(home).join(".asoundrc");

    fs::write(path, entries).expect("should be able to write to $HOME/.asoundrc");
}

fn generate_asoundrc_entry(name: &str, loopback_card_id: u8) -> String {
    format!(
        r#"
pcm.{name} {{
        type plug
        slave {{
                pcm "dmix:PCH,{loopback_card_id}"
        }}
        hint {{
                show on
                description "dummy device"
        }}
}}
ctl.{name} {{
        type dmix
}}"#
    )
}
