use std::path::{Path, PathBuf};

const DEV_DIR: &str = "dev";
const PROD_DIR: &str = "prod";

pub fn audio_data_dir() -> PathBuf {
    parent_dir().join("audio")
}

pub fn state_recovery_file_path() -> PathBuf {
    parent_dir().join("state-recovery-info")
}

fn parent_dir<'a>() -> &'a Path {
    if cfg!(not(debug_assertions)) {
        Path::new(DEV_DIR)
    } else {
        Path::new(PROD_DIR)
    }
}
