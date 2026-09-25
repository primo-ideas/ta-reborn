//! Relay server: pairs players into matches and clocks the lockstep turns.
//!
//! The server never simulates anything. Each match collects the commands sent by its
//! players and broadcasts them as one [`Turn`] every [`TURN_DURATION`]; every client
//! applies the same turns, so they all compute the same game.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use ta_net::{
    ClientMsg, PLAYERS_PER_MATCH, PROTOCOL_VERSION, PlayerId, ServerMsg, TURN_DURATION, Turn,
    decode, encode,
};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;

type WsSink = SplitSink<WebSocketStream<TcpStream>, Message>;
type WsStream = SplitStream<WebSocketStream<TcpStream>>;
/// A message from a player to its match; `None` when the player disconnected.
type Inbound = (PlayerId, Option<ClientMsg>);

/// A player's place in a match, handed to its connection when the match starts.
struct Seat {
    player: PlayerId,
    to_match: mpsc::UnboundedSender<Inbound>,
    from_match: mpsc::UnboundedReceiver<ServerMsg>,
}

/// Players waiting for a match.
type Lobby = Arc<Mutex<Vec<oneshot::Sender<Seat>>>>;

pub async fn serve(listener: TcpListener) -> std::io::Result<()> {
    let lobby = Lobby::default();
    loop {
        let (stream, _) = listener.accept().await?;
        tokio::spawn(handle_connection(stream, lobby.clone()));
    }
}

async fn handle_connection(stream: TcpStream, lobby: Lobby) {
    let Ok(ws) = tokio_tungstenite::accept_async(stream).await else {
        return;
    };
    let (mut sink, mut stream) = ws.split();

    match recv(&mut stream).await {
        Some(ClientMsg::Join { version }) if version == PROTOCOL_VERSION => {}
        Some(ClientMsg::Join { .. }) => {
            send(
                &mut sink,
                &ServerMsg::VersionMismatch {
                    server: PROTOCOL_VERSION,
                },
            )
            .await;
            return;
        }
        _ => return,
    }

    let (seat_tx, seat_rx) = oneshot::channel();
    join_lobby(&lobby, seat_tx);
    send(&mut sink, &ServerMsg::Waiting).await;
    let mut seat = tokio::select! {
        seat = seat_rx => match seat {
            Ok(seat) => seat,
            Err(_) => return,
        },
        // The client has nothing to say before the match starts: anything means it left.
        _ = recv(&mut stream) => return,
    };

    loop {
        tokio::select! {
            msg = seat.from_match.recv() => match msg {
                Some(msg) => {
                    if !send(&mut sink, &msg).await {
                        break;
                    }
                }
                None => break,
            },
            msg = recv(&mut stream) => match msg {
                Some(msg) => {
                    let _ = seat.to_match.send((seat.player, Some(msg)));
                }
                None => break,
            },
        }
    }
    let _ = seat.to_match.send((seat.player, None));
}

fn join_lobby(lobby: &Lobby, seat_tx: oneshot::Sender<Seat>) {
    let mut waiting = lobby.lock().unwrap();
    // Forget players who left while waiting.
    waiting.retain(|seat_tx| !seat_tx.is_closed());
    waiting.push(seat_tx);
    if waiting.len() == PLAYERS_PER_MATCH as usize {
        start_match(waiting.drain(..).collect());
    }
}

fn start_match(players: Vec<oneshot::Sender<Seat>>) {
    let (to_match, inbox) = mpsc::unbounded_channel();
    let mut outboxes = Vec::new();
    for (player, seat_tx) in players.into_iter().enumerate() {
        let player = player as PlayerId;
        let (outbox, from_match) = mpsc::unbounded_channel();
        outboxes.push(outbox);
        let seat = Seat {
            player,
            to_match: to_match.clone(),
            from_match,
        };
        if seat_tx.send(seat).is_err() {
            // Left at the last moment: the match sees it as a disconnection.
            let _ = to_match.send((player, None));
        }
    }
    tokio::spawn(run_match(outboxes, inbox));
}

/// Runs until every player has disconnected.
async fn run_match(
    players: Vec<mpsc::UnboundedSender<ServerMsg>>,
    mut inbox: mpsc::UnboundedReceiver<Inbound>,
) {
    let broadcast = |msg: ServerMsg| {
        for player in &players {
            let _ = player.send(msg.clone());
        }
    };
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    for (you, player) in players.iter().enumerate() {
        let players = players.len() as u8;
        let _ = player.send(ServerMsg::Start {
            seed,
            players,
            you: you as PlayerId,
        });
    }

    let mut ticker = tokio::time::interval(TURN_DURATION);
    let mut turn = 0;
    let mut commands = Vec::new();
    // First checksum received for each turn, and how many players reported it.
    let mut checksums: BTreeMap<u32, (u64, usize)> = BTreeMap::new();
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                broadcast(ServerMsg::Turn(Turn { number: turn, commands: std::mem::take(&mut commands) }));
                turn += 1;
            }
            inbound = inbox.recv() => match inbound {
                None => return,
                Some((player, None)) => broadcast(ServerMsg::PlayerLeft { player }),
                Some((player, Some(ClientMsg::Command(command)))) => commands.push((player, command)),
                Some((_, Some(ClientMsg::Checksum { turn, hash }))) => {
                    let (first, reports) = checksums.entry(turn).or_insert((hash, 0));
                    if *first != hash {
                        broadcast(ServerMsg::Desync { turn });
                    }
                    *reports += 1;
                    if *reports == players.len() {
                        checksums.remove(&turn);
                    }
                }
                Some((_, Some(ClientMsg::Join { .. }))) => {}
            },
        }
    }
}

/// Next protocol message, skipping control frames. `None` once the client is gone or
/// sent something undecodable.
async fn recv(stream: &mut WsStream) -> Option<ClientMsg> {
    loop {
        match stream.next().await?.ok()? {
            Message::Binary(bytes) => return decode(&bytes).ok(),
            Message::Close(_) => return None,
            _ => {}
        }
    }
}

async fn send(sink: &mut WsSink, msg: &ServerMsg) -> bool {
    sink.send(Message::Binary(encode(msg).into())).await.is_ok()
}
