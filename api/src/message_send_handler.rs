use actix::{dev::ToEnvelope, Actor, Addr, Handler, Message};
use std::time::{Duration, Instant};

pub trait MessageLimiter<M>: Send
where
    M: Message + Send,
    M::Result: Send,
{
    fn can_send(&self, msg: &M) -> bool;

    fn has_sent(&mut self, msg: &M);
}

pub struct MessageSendHandler<M>
where
    M: Message + Send,
    M::Result: Send,
{
    limiters: Vec<Box<dyn MessageLimiter<M>>>,
}

impl<M> MessageSendHandler<M>
where
    M: Message + Send,
    M::Result: Send,
{
    pub fn with_limiters(limiters: Vec<Box<dyn MessageLimiter<M>>>) -> Self {
        Self { limiters }
    }

    pub fn send_msg<H>(&mut self, msg: M, addr: &Addr<H>)
    where
        H: Handler<M>,
        <H as Actor>::Context: ToEnvelope<H, M>,
    {
        let can_send = self
            .limiters
            .iter()
            .map(|l| l.can_send(&msg))
            .reduce(|acc, x| acc && x)
            .unwrap_or(false);

        if can_send {
            self.limiters.iter_mut().for_each(|l| l.has_sent(&msg));
            addr.do_send(msg);
        }
    }
}

#[derive(Debug, Clone)]
pub struct RateLimiter {
    last_msg_sent_at: Instant,
    hard_rate_limit: Duration,
}

#[derive(Debug, Clone)]
pub struct ChangeDetector<M>
where
    M: Message + Send + PartialEq + Clone,
    M::Result: Send,
{
    last_msg_sent: Option<M>,
}

impl<M> MessageLimiter<M> for RateLimiter
where
    M: Message + Send,
    M::Result: Send,
{
    fn can_send(&self, _msg: &M) -> bool {
        Instant::now().duration_since(self.last_msg_sent_at) > self.hard_rate_limit
    }

    fn has_sent(&mut self, _msg: &M) {
        self.last_msg_sent_at = Instant::now();
    }
}

impl<M> MessageLimiter<M> for ChangeDetector<M>
where
    M: Message + Send + PartialEq + Clone,
    M::Result: Send,
{
    fn can_send(&self, msg: &M) -> bool {
        self.last_msg_sent
            .as_ref()
            .map(|lms| lms != msg)
            .unwrap_or(true)
    }

    fn has_sent(&mut self, msg: &M) {
        self.last_msg_sent = Some(msg.clone())
    }
}

impl RateLimiter {
    pub fn with_rate_limit(rate_limit: Duration) -> Self {
        Self {
            last_msg_sent_at: Instant::now(),
            hard_rate_limit: rate_limit,
        }
    }
}

impl<M> ChangeDetector<M>
where
    M: Message + Send + PartialEq + Clone,
    M::Result: Send,
{
    pub fn new(default_msg: Option<M>) -> Self {
        Self {
            last_msg_sent: default_msg,
        }
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self {
            last_msg_sent_at: Instant::now(),
            hard_rate_limit: Duration::from_millis(33),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::tests_utils::{GetReceivedMessageCount, TestMessage, TestMessageHandler};

    use super::*;

    #[actix_web::test]
    async fn test_change_notifier() {
        {
            let test_handler = TestMessageHandler::new(Some("test".into()));
            let addr = test_handler.start();

            let mut msg_handler = MessageSendHandler::with_limiters(vec![Box::new(
                ChangeDetector::<TestMessage>::new(None),
            )]);

            msg_handler.send_msg("test".into(), &addr);

            msg_handler.send_msg("test".into(), &addr); // will not be received due to change

            let msg_count = addr.send(GetReceivedMessageCount).await.unwrap();
            pretty_assertions::assert_eq!(msg_count, 1);
        }

        {
            let test_handler = TestMessageHandler::new(Some("test".into()));
            let addr = test_handler.start();

            let mut msg_handler = MessageSendHandler::with_limiters(vec![Box::new(
                ChangeDetector::new(Some(TestMessage {
                    value: "test".to_owned(),
                })),
            )]);

            msg_handler.send_msg("test".into(), &addr);

            let msg_count = addr.send(GetReceivedMessageCount).await.unwrap();
            pretty_assertions::assert_eq!(msg_count, 0);
        }

        {
            let test_handler = TestMessageHandler::new(None);
            let addr = test_handler.start();

            let mut msg_handler = MessageSendHandler::with_limiters(vec![Box::new(
                ChangeDetector::<TestMessage>::new(None),
            )]);

            msg_handler.send_msg("test 1".into(), &addr); // send
            msg_handler.send_msg("test 2".into(), &addr); // send
            msg_handler.send_msg("test 1".into(), &addr); // send
            msg_handler.send_msg("test 1".into(), &addr); // ignore

            let msg_count = addr.send(GetReceivedMessageCount).await.unwrap();
            pretty_assertions::assert_eq!(msg_count, 3);
        }
    }

    #[actix_web::test]
    async fn test_rate_limiter_and_change_notifier() {
        {
            let test_handler = TestMessageHandler::new(Some("test".into()));
            let addr = test_handler.start();

            let mut msg_handler = MessageSendHandler::with_limiters(vec![
                Box::new(ChangeDetector::<TestMessage>::new(None)),
                Box::new(RateLimiter::with_rate_limit(Duration::from_millis(50))),
            ]);

            std::thread::sleep(Duration::from_millis(50));

            msg_handler.send_msg("test".into(), &addr);

            msg_handler.send_msg("test".into(), &addr); // will not be received due to change
            msg_handler.send_msg("abc".into(), &addr); // will not be received due to rate limit

            let msg_count = addr.send(GetReceivedMessageCount).await.unwrap();
            pretty_assertions::assert_eq!(msg_count, 1);
        }
    }
}
