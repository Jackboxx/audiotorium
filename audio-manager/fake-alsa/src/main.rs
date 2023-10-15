use std::{env, fs, path::PathBuf};

fn main() {
    let entries = (0..3)
        .map(|i| generate_asoundrc_entry(i))
        .collect::<Vec<_>>()
        .join("\n\n");

    let home = env::var("HOME").expect("HOME env var should exits");
    let path = PathBuf::from(home).join(".asoundrc");

    fs::write(path, entries).expect("should be able to write to $HOME/.asoundrc");
}

fn generate_asoundrc_entry(index: u8) -> String {
    format!(
        r#"pcm.dummy_{index} {{
        type plug
        slave {{
                pcm "hw:Loopback,0,{index}"
        }}
        hint {{
                show on
                description "dummy device"
        }}
}}
ctl.dummy_{index} {{
        type dmix
}}"#
    )
}
