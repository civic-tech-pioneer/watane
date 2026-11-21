//! Events to pages (TECHNICAL 9.3): the page bus and its SSE adapter, which
//! turns lag into one `changed` message and ends streams on shutdown.

use crate::LocalName;
use crate::agent::streams::{ConnGuard, ConnPoll};
use axum::response::sse::{Event, KeepAlive};
use axum::response::{IntoResponse, Response, Sse};
use serde_json::{Value, json};
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::sync::{broadcast, watch};
use tokio_stream::Stream;
use tokio_stream::wrappers::{BroadcastStream, WatchStream};

type Message = (LocalName, Arc<str>);
pub(crate) const CLOSE: &str = "";
pub(crate) const CHANGED: &str = r#"{"type":"changed"}"#;
pub(crate) const CATALOG: &str = r#"{"type":"catalog"}"#;
pub(crate) const RELOAD: &str = r#"{"type":"reload"}"#;

pub(crate) struct Emission {
    pub interface: String,
    pub function: String,
    pub args: Value,
}

#[derive(Debug)]
pub(crate) struct PageBus {
    sender: Mutex<broadcast::Sender<Message>>,
}
impl PageBus {
    pub fn new(capacity: usize) -> Self {
        Self {
            sender: Mutex::new(broadcast::channel(capacity).0),
        }
    }
    // Serialize channel subscription and publication. Per-mod queues preserve commit order.
    pub fn publication(&self) -> MutexGuard<'_, broadcast::Sender<Message>> {
        self.sender
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
    }
    pub fn send(&self, local: &LocalName, message: impl Into<Arc<str>>) {
        let _ = self.publication().send((local.clone(), message.into()));
    }
    #[cfg(test)]
    pub fn response(
        &self,
        local: LocalName,
        shutdown: watch::Receiver<bool>,
        interval: Duration,
    ) -> Response {
        self.page_response(local, shutdown, interval, None)
    }
    /// A response that also carries a page connection: its id first, then its stream items (8.4).
    pub fn response_with_connection(
        &self,
        local: LocalName,
        shutdown: watch::Receiver<bool>,
        interval: Duration,
        connection: ConnGuard,
    ) -> Response {
        self.page_response(local, shutdown, interval, Some(connection))
    }
    fn page_response(
        &self,
        local: LocalName,
        shutdown: watch::Receiver<bool>,
        interval: Duration,
        connection: Option<ConnGuard>,
    ) -> Response {
        let stream = PageStream {
            receiver: BroadcastStream::new(self.publication().subscribe()),
            shutdown: WatchStream::new(shutdown),
            local,
            connection,
            announced: false,
            turn: false,
        };
        Sse::new(stream)
            .keep_alive(KeepAlive::new().interval(interval).text("ping"))
            .into_response()
    }
}

pub(crate) fn event_message(emitter: &LocalName, event: &crate::EventRecord) -> String {
    json!({"type":"event", "from":emitter, "interface":event.interface,
        "function":event.function, "args":event.args})
    .to_string()
}

struct PageStream {
    receiver: BroadcastStream<Message>,
    shutdown: WatchStream<bool>,
    local: LocalName,
    /// Step 11: the connection of this response and its lossless queue (9.3).
    connection: Option<ConnGuard>,
    announced: bool,
    turn: bool,
}
enum Got {
    Message(Event),
    End,
    Nothing,
}
impl PageStream {
    fn bus(&mut self, cx: &mut Context<'_>) -> Got {
        loop {
            match Pin::new(&mut self.receiver).poll_next(cx) {
                Poll::Ready(Some(Ok((local, message)))) if local == self.local => {
                    if message.is_empty() {
                        return Got::End;
                    }
                    return Got::Message(Event::default().data(message.as_ref()));
                }
                Poll::Ready(Some(Ok(_))) => continue,
                Poll::Ready(Some(Err(_))) => return Got::Message(Event::default().data(CHANGED)),
                Poll::Ready(None) => return Got::End,
                Poll::Pending => return Got::Nothing,
            }
        }
    }
    fn queue(&mut self, cx: &mut Context<'_>) -> Got {
        let Some(connection) = &self.connection else {
            return Got::Nothing;
        };
        match connection.poll(cx.waker()) {
            ConnPoll::Message(message) => Got::Message(Event::default().data(message)),
            ConnPoll::Pending => Got::Nothing,
            ConnPoll::Closed => Got::End,
        }
    }
}
impl Stream for PageStream {
    type Item = Result<Event, Infallible>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match Pin::new(&mut self.shutdown).poll_next(cx) {
                Poll::Ready(Some(false)) => continue,
                Poll::Ready(_) => return Poll::Ready(None),
                Poll::Pending => break,
            }
        }
        if !self.announced
            && let Some(connection) = &self.connection
        {
            let message = format!(r#"{{"type":"connection","id":"{}"}}"#, connection.id());
            self.announced = true;
            return Poll::Ready(Some(Ok(Event::default().data(message))));
        }
        // Alternate, so that neither the bus nor the queue starves the other.
        self.turn = !self.turn;
        let order = if self.turn {
            [true, false]
        } else {
            [false, true]
        };
        for bus in order {
            let got = if bus { self.bus(cx) } else { self.queue(cx) };
            match got {
                Got::Message(event) => return Poll::Ready(Some(Ok(event))),
                Got::End => return Poll::Ready(None),
                Got::Nothing => {}
            }
        }
        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_stream::StreamExt;

    #[tokio::test]
    async fn ac6_lag_replaces_lost_messages_with_one_changed() {
        let bus = PageBus::new(2);
        let local = LocalName::parse("counter").unwrap();
        let (_stop, rx) = watch::channel(false);
        let mut body = bus
            .response(local.clone(), rx, Duration::from_secs(15))
            .into_body()
            .into_data_stream();
        for message in [
            "lost",
            "also lost",
            r#"{"type":"reload"}"#,
            r#"{"type":"changed"}"#,
        ] {
            bus.send(&local, message);
        }
        for expected in [
            "data: {\"type\":\"changed\"}\n\n",
            "data: {\"type\":\"reload\"}\n\n",
            "data: {\"type\":\"changed\"}\n\n",
        ] {
            assert_eq!(
                body.next().await.unwrap().unwrap().as_ref(),
                expected.as_bytes()
            );
        }
        tokio::time::timeout(Duration::from_millis(20), body.next())
            .await
            .unwrap_err();
    }

    #[tokio::test]
    async fn ac7_keep_alive_pings_after_idle_interval_and_shutdown_ends_stream() {
        let bus = PageBus::new(2);
        let local = LocalName::parse("counter").unwrap();
        let (stop, rx) = watch::channel(false);
        let mut body = bus
            .response(local.clone(), rx, Duration::from_millis(80))
            .into_body()
            .into_data_stream();
        tokio::time::timeout(Duration::from_millis(20), body.next())
            .await
            .unwrap_err();
        bus.send(&local, r#"{"type":"changed"}"#);
        assert_eq!(
            body.next().await.unwrap().unwrap().as_ref(),
            b"data: {\"type\":\"changed\"}\n\n"
        );
        tokio::time::timeout(Duration::from_millis(20), body.next())
            .await
            .unwrap_err();
        assert_eq!(
            tokio::time::timeout(Duration::from_millis(150), body.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap()
                .as_ref(),
            b": ping\n\n"
        );
        stop.send(true).unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(50), body.next())
                .await
                .unwrap()
                .is_none()
        );
    }
}
