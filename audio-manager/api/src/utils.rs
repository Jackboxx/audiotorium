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

#[derive(Debug, Clone)]
pub struct ChangeNotifier<M>
where
    M: Message + Send + Clone + PartialEq,
    M::Result: Send,
{
    last_msg_sent: Option<M>,
}

impl MessageRateLimiter {
    pub fn send_msg<M, H>(&mut self, msg: M, addr: &Addr<H>)
    where
        M: Message + Send,
        M::Result: Send,
        H: Handler<M>,
        <H as Actor>::Context: ToEnvelope<H, M>,
    {
        if Instant::now().duration_since(self.last_msg_sent_at) > self.hard_rate_limit {
            self.last_msg_sent_at = Instant::now();
            addr.do_send(msg);
        }
    }
}

impl<M> ChangeNotifier<M>
where
    M: Message + Send + Clone + PartialEq,
    M::Result: Send,
{
    pub fn new(default_msg: Option<M>) -> Self {
        Self {
            last_msg_sent: default_msg,
        }
    }

    pub fn send_msg<H>(&mut self, msg: M, addr: &Addr<H>)
    where
        H: Handler<M>,
        <H as Actor>::Context: ToEnvelope<H, M>,
    {
        let option_msg = Some(msg.clone());
        if option_msg != self.last_msg_sent {
            self.last_msg_sent = option_msg;
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

#[cfg(test)]
mod tests {
    use crate::tests_utils::{GetReceivedMessageCount, TestMessage, TestMessageHandler};

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

    #[actix_web::test]
    async fn test_change_notifier() {
        {
            let test_handler = TestMessageHandler::new(Some("test".into()));
            let addr = test_handler.start();

            let mut notifier = ChangeNotifier::<TestMessage>::new(None);
            notifier.send_msg("test".into(), &addr);

            notifier.send_msg("test".into(), &addr); // will not be received due to change

            let msg_count = addr.send(GetReceivedMessageCount).await.unwrap();
            pretty_assertions::assert_eq!(msg_count, 1);
        }

        {
            let test_handler = TestMessageHandler::new(Some("test".into()));
            let addr = test_handler.start();

            let mut notifier = ChangeNotifier::<TestMessage>::new(Some("test".into()));
            notifier.send_msg("test".into(), &addr);

            let msg_count = addr.send(GetReceivedMessageCount).await.unwrap();
            pretty_assertions::assert_eq!(msg_count, 0);
        }

        {
            let test_handler = TestMessageHandler::new(None);
            let addr = test_handler.start();

            let mut notifier = ChangeNotifier::<TestMessage>::new(None);
            notifier.send_msg("test 1".into(), &addr); // send
            notifier.send_msg("test 2".into(), &addr); // send
            notifier.send_msg("test 1".into(), &addr); // send
            notifier.send_msg("test 1".into(), &addr); // ignore

            let msg_count = addr.send(GetReceivedMessageCount).await.unwrap();
            pretty_assertions::assert_eq!(msg_count, 3);
        }
    }
}
