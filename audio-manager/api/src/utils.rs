use std::{collections::HashMap, fs};

use actix::Addr;
use anyhow::anyhow;
use cpal::{
    traits::{DeviceTrait, HostTrait},
    Device, SampleRate, StreamConfig,
};
use serde::{Deserialize, Serialize};

use crate::{
    brain::brain_server::{AudioBrain, GetAudioNodeMessage},
    node::node_server::AudioNode,
};

const DEFAULT_SAMPLE_RATE: u32 = 48000;

pub async fn get_node_by_source_name(
    source_name: String,
    addr: &Addr<AudioBrain>,
) -> Option<Addr<AudioNode>> {
    addr.send(GetAudioNodeMessage { source_name }).await.ok()?
}

pub fn setup_device(source_name: &str) -> anyhow::Result<(Device, StreamConfig)> {
    let host = cpal::default_host();
    let device = host
        .output_devices()?
        .find(|dev| dev.name().map(|v| v == source_name).unwrap_or(false))
        .ok_or(anyhow!("no device with source name {source_name} found"))?;

    let mut supported_configs_range = device.supported_output_configs()?;

    let supported_config = supported_configs_range
        .next()
        .ok_or(anyhow!("no config found"))?;

    let channel_count = 2; // I choose to make this assumption not because it is good
                           // but because it is easy

    let config = supported_config
        .with_sample_rate(SampleRate(DEFAULT_SAMPLE_RATE * channel_count))
        .into();

    Ok((device, config))
}

pub fn type_as_str<'a, T: Sized>(_v: &T) -> &'a str {
    std::any::type_name::<T>()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSourceInfo {
    pub human_readable_name: String,
}

pub type Sources = HashMap<String, AudioSourceInfo>;

pub fn get_audio_sources() -> Sources {
    let source_str = if cfg!(not(debug_assertions)) {
        fs::read_to_string("sources-prod.toml")
            .expect("'sources-prod.toml' should exist in production env")
    } else {
        fs::read_to_string("sources-dev.toml")
            .expect("'sources-dev.toml' should exist in development env")
    };

    toml::from_str(&source_str).expect("sources file should be valid toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_as_str() {
        let input = type_as_str(&"hello");
        pretty_assertions::assert_eq!(input, "&str");

        struct TestStruct;
        let input = type_as_str(&TestStruct);
        pretty_assertions::assert_eq!(
            input,
            "audio_manager_api::utils::tests::test_type_as_str::TestStruct"
        )
    }
}
