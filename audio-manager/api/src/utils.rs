use std::time::{Duration, Instant};

use actix::{dev::ToEnvelope, Actor, Addr, Handler, Message};
use anyhow::anyhow;
use cpal::{
    traits::{DeviceTrait, HostTrait},
    Device, SampleRate, StreamConfig,
};

use crate::{
    brain::brain_server::{AudioBrain, GetAudioNodeMessage},
    node::node_server::AudioNode,
};

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

    pub fn send_msg_urgent<M, H>(&mut self, msg: M, addr: Option<&Addr<H>>)
    where
        M: Message + Send,
        M::Result: Send,
        H: Handler<M>,
        <H as Actor>::Context: ToEnvelope<H, M>,
    {
        let Some(addr) = addr else { return };

        self.last_msg_sent_at = Instant::now();
        addr.do_send(msg);
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
