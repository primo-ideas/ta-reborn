//! Deterministic simulation of a match.
//!
//! Every client runs this simulation in lockstep and must stay bit-identical, on
//! native and on wasm alike. Rules for code in this crate:
//! - no floating point: use [`Fx`] (conversions to/from `f32` are for the client only);
//! - no iteration over `HashMap`/`HashSet`, whose order is not stable;
//! - no randomness other than values derived from the match seed;
//! - no Bevy, no wall-clock time.

mod fixed;
mod sim;
mod terrain;

pub use fixed::{Fx, FxVec2};
pub use sim::{Command, MAX_PLAYERS, PlayerId, Sim, TICK_RATE, Unit, UnitId};
pub use terrain::Terrain;
