use actix::Addr;
use cpal::{
    traits::{DeviceTrait, HostTrait},
    SampleRate,
};

use crate::{audio::AudioSource, server::QueueServer};

const DEFAULT_SAMPLE_RATE: u32 = 48000;

/// TODO: Handle errors
pub fn create_source(source_name: &str, server_addr: Addr<QueueServer>) -> AudioSource {
    let host = cpal::default_host();
    let device = host
        .output_devices()
        .expect("no output device available")
        .find(|dev| dev.name().expect("device has no name") == source_name)
        .expect("no device found");

    let mut supported_configs_range = device
        .supported_output_configs()
        .expect("error while querying configs");

    let supported_config = supported_configs_range
        .next()
        .expect("no supported config?!");

    let channel_count = 2; // I choose to make this assumption not because it is good
                           // but because it is easy

    let config = supported_config
        .with_sample_rate(SampleRate(DEFAULT_SAMPLE_RATE * channel_count))
        .into();

    AudioSource::new(device, config, Vec::new(), server_addr)
}
