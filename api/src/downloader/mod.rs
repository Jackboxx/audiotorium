pub mod actor;
pub mod download_identifier;
pub mod info;
mod youtube;

#[cfg(not(debug_assertions))]
pub const AUDIO_DIR: &str = "audio";

#[cfg(debug_assertions)]
pub const AUDIO_DIR: &str = "audio-dev";
