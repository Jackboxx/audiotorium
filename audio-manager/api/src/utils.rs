use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use actix::{dev::ToEnvelope, Actor, Addr, Handler, Message};
use cpal::{
    traits::{DeviceTrait, HostTrait},
    SampleRate,
};

use crate::audio_player::AudioPlayer;

const DEFAULT_SAMPLE_RATE: u32 = 48000;

#[derive(Debug, Clone)]
pub struct MessageRateLimiter {
    last_msg_sent_at: Instant,
    hard_rate_limit: Duration,
}

impl MessageRateLimiter {
    pub fn send_msg<M, H>(&mut self, msg: M, addr: Option<&Addr<H>>)
    where
        M: Message + Send,
        M::Result: Send,
        H: Handler<M>,
        <H as Actor>::Context: ToEnvelope<H, M>,
    {
        let Some(addr) = addr else { return };

        if Instant::now().duration_since(self.last_msg_sent_at) > self.hard_rate_limit {
            self.last_msg_sent_at = Instant::now();
            addr.do_send(msg);
        }
    }
}

impl Default for MessageRateLimiter {
    fn default() -> Self {
        Self {
            last_msg_sent_at: Instant::now(),
            hard_rate_limit: Duration::from_millis(33),
        }
    }
}

/// TODO: Handle errors
pub fn create_player(source_name: &str) -> AudioPlayer<PathBuf> {
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

    AudioPlayer::new(device, config, None)
}
