use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use ta_net::{ClientMsg, Command, PROTOCOL_VERSION, ServerMsg, Turn, decode, encode};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Client = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn start_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    tokio::spawn(ta_server::serve(listener));
    url
}

async fn join(url: &str, version: u32) -> Client {
    let (mut ws, _) = tokio_tungstenite::connect_async(url).await.unwrap();
    send(&mut ws, ClientMsg::Join { version }).await;
    ws
}

async fn send(ws: &mut Client, msg: ClientMsg) {
    ws.send(Message::Binary(encode(&msg).into())).await.unwrap();
}

async fn recv(ws: &mut Client) -> ServerMsg {
    loop {
        if let Message::Binary(bytes) = ws.next().await.unwrap().unwrap() {
            return decode(&bytes).unwrap();
        }
    }
}

async fn next_turn_with_commands(ws: &mut Client) -> Turn {
    loop {
        if let ServerMsg::Turn(turn) = recv(ws).await
            && !turn.commands.is_empty()
        {
            return turn;
        }
    }
}

async fn with_timeout(test: impl Future<Output = ()>) {
    tokio::time::timeout(Duration::from_secs(10), test)
        .await
        .expect("test timed out");
}

#[tokio::test]
async fn players_of_a_match_receive_the_same_turns() {
    with_timeout(async {
        let url = start_server().await;
        let mut a = join(&url, PROTOCOL_VERSION).await;
        assert_eq!(recv(&mut a).await, ServerMsg::Waiting);
        let mut b = join(&url, PROTOCOL_VERSION).await;
        assert_eq!(recv(&mut b).await, ServerMsg::Waiting);

        let (
            ServerMsg::Start {
                seed: seed_a,
                players: 2,
                you: 0,
            },
            ServerMsg::Start {
                seed: seed_b,
                players: 2,
                you: 1,
            },
        ) = (recv(&mut a).await, recv(&mut b).await)
        else {
            panic!("expected both players to start");
        };
        assert_eq!(seed_a, seed_b);

        let command = Command::Stop { units: vec![] };
        send(&mut b, ClientMsg::Command(command.clone())).await;
        let turn = next_turn_with_commands(&mut a).await;
        assert_eq!(turn.commands, vec![(1, command)]);
        assert_eq!(next_turn_with_commands(&mut b).await, turn);

        drop(b);
        while recv(&mut a).await != (ServerMsg::PlayerLeft { player: 1 }) {}
    })
    .await;
}

#[tokio::test]
async fn rejects_other_protocol_versions() {
    with_timeout(async {
        let url = start_server().await;
        let mut client = join(&url, PROTOCOL_VERSION + 1).await;
        assert_eq!(
            recv(&mut client).await,
            ServerMsg::VersionMismatch {
                server: PROTOCOL_VERSION
            }
        );
    })
    .await;
}
