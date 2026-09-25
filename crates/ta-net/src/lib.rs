//! Messages exchanged between clients and the relay server, and the lockstep turn
//! scheduler shared by solo and online games.

mod lockstep;

use std::time::Duration;

use serde::{Deserialize, Serialize};
use ta_sim::TICK_RATE;

pub use lockstep::Lockstep;
pub use ta_sim::{Command, PlayerId};

/// Must be bumped whenever messages or simulation rules change: players of a match
/// have to run identical code.
pub const PROTOCOL_VERSION: u32 = 1;
pub const DEFAULT_PORT: u16 = 7878;
pub const PLAYERS_PER_MATCH: u8 = 2;
/// Commands are grouped into turns of 3 ticks (100 ms).
pub const TICKS_PER_TURN: u32 = 3;
pub const TURN_DURATION: Duration =
    Duration::from_millis((TICKS_PER_TURN * 1000 / TICK_RATE) as u64);
/// Clients report a checksum every this many turns (once per second).
pub const CHECKSUM_INTERVAL: u32 = 10;

/// The commands applied at the start of a turn, in order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Turn {
    pub number: u32,
    pub commands: Vec<(PlayerId, Command)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ClientMsg {
    Join { version: u32 },
    Command(Command),
    Checksum { turn: u32, hash: u64 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ServerMsg {
    /// Joined; waiting for enough players to start a match.
    Waiting,
    VersionMismatch {
        server: u32,
    },
    Start {
        seed: u64,
        players: u8,
        you: PlayerId,
    },
    Turn(Turn),
    /// Players reported different checksums after this turn.
    Desync {
        turn: u32,
    },
    PlayerLeft {
        player: PlayerId,
    },
}

pub fn encode<T: Serialize>(msg: &T) -> Vec<u8> {
    postcard::to_allocvec(msg).expect("protocol messages always serialize")
}

pub fn decode<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Result<T, postcard::Error> {
    postcard::from_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ta_sim::{Fx, FxVec2, UnitId};

    #[test]
    fn messages_round_trip() {
        let target = FxVec2::new(Fx::from_f32(12.5), Fx::from_int(-3));
        let msg = ServerMsg::Turn(Turn {
            number: 7,
            commands: vec![(
                1,
                Command::Move {
                    units: vec![UnitId(3), UnitId(4)],
                    target,
                },
            )],
        });
        assert_eq!(decode::<ServerMsg>(&encode(&msg)).unwrap(), msg);
    }
}
