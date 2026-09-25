use bevy::prelude::*;
use ewebsock::{Options, WsEvent, WsMessage, WsReceiver, WsSender};
use ta_net::{ClientMsg, ServerMsg, decode, encode};

/// Drops the connection, if any, which closes it.
pub fn close_connection(commands: &mut Commands) {
    commands.queue(|world: &mut World| {
        world.remove_non_send::<Connection>();
    });
}

/// WebSocket connection to the relay server. On the web the socket is a JS object and
/// is not `Send`, so this lives in a non-send resource.
pub struct Connection {
    sender: WsSender,
    receiver: WsReceiver,
}

pub enum NetEvent {
    Opened,
    Message(ServerMsg),
    Closed(String),
}

impl Connection {
    pub fn open(url: &str) -> Result<Connection, String> {
        let (sender, receiver) = ewebsock::connect(url, Options::default())?;
        Ok(Connection { sender, receiver })
    }

    pub fn send(&mut self, msg: &ClientMsg) {
        self.sender.send(WsMessage::Binary(encode(msg)));
    }

    /// Next event received, without blocking.
    pub fn poll(&mut self) -> Option<NetEvent> {
        loop {
            return Some(match self.receiver.try_recv()? {
                WsEvent::Opened => NetEvent::Opened,
                WsEvent::Message(WsMessage::Binary(bytes)) => match decode(&bytes) {
                    Ok(msg) => NetEvent::Message(msg),
                    Err(err) => NetEvent::Closed(format!("invalid message from server: {err}")),
                },
                WsEvent::Message(_) => continue,
                WsEvent::Error(err) => NetEvent::Closed(err),
                WsEvent::Closed => NetEvent::Closed("connection closed".into()),
            });
        }
    }
}
