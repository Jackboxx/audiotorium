use actix::{Actor, Context, Handler, Message};

#[derive(Debug, Clone, PartialEq, Eq, Message)]
#[rtype(result = "()")]
pub struct TestMessage {
    pub value: String,
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "usize")]
pub struct GetReceivedMessageCount;

pub struct TestMessageHandler {
    msgs_received: usize,
    expected_msg: Option<TestMessage>,
}
impl From<&str> for TestMessage {
    fn from(value: &str) -> Self {
        Self {
            value: value.to_owned(),
        }
    }
}

impl From<String> for TestMessage {
    fn from(value: String) -> Self {
        Self { value }
    }
}

impl TestMessageHandler {
    pub fn new(expected_msg: Option<TestMessage>) -> Self {
        Self {
            msgs_received: 0,
            expected_msg,
        }
    }
}

impl Actor for TestMessageHandler {
    type Context = Context<Self>;
}

impl Handler<TestMessage> for TestMessageHandler {
    type Result = ();

    fn handle(&mut self, msg: TestMessage, _ctx: &mut Self::Context) -> Self::Result {
        if let Some(expected_msg) = self.expected_msg.as_ref() {
            pretty_assertions::assert_eq!(&msg, expected_msg);
        }

        self.msgs_received += 1;
    }
}

impl Handler<GetReceivedMessageCount> for TestMessageHandler {
    type Result = usize;

    fn handle(&mut self, _msg: GetReceivedMessageCount, _ctx: &mut Self::Context) -> Self::Result {
        self.msgs_received
    }
}
