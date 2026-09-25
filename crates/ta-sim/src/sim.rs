use serde::{Deserialize, Serialize};

use crate::{Fx, FxVec2, Terrain};

/// Simulation ticks per second (Total Annihilation also ran its game logic at 30 Hz).
pub const TICK_RATE: u32 = 30;
pub const MAX_PLAYERS: u8 = 2;

const MAP_CELLS: u32 = 128;
const UNITS_PER_PLAYER: usize = 6;
/// Distance covered per tick: 0.2 m, i.e. 6 m/s.
const UNIT_SPEED: Fx = Fx::ratio(1, 5);
/// Distance between neighbours in a formation.
const UNIT_SPACING: Fx = Fx::from_int(4);

pub type PlayerId = u8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct UnitId(pub u32);

#[derive(Clone, Debug)]
pub struct Unit {
    pub id: UnitId,
    pub owner: PlayerId,
    pub pos: FxVec2,
    /// Destination of the current move order.
    pub target: Option<FxVec2>,
}

/// An order from a player. Commands come from the network, so they are validated.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    Move { units: Vec<UnitId>, target: FxVec2 },
    Stop { units: Vec<UnitId> },
}

pub struct Sim {
    tick: u64,
    terrain: Terrain,
    /// Sorted by id.
    units: Vec<Unit>,
}

impl Sim {
    pub fn new(seed: u64, players: u8) -> Sim {
        assert!(
            (1..=MAX_PLAYERS).contains(&players),
            "unsupported player count"
        );
        let terrain = Terrain::generate(seed, MAP_CELLS);
        let map = terrain.world_size();
        let mut units = Vec::new();
        for owner in 0..players {
            // Armies start on the west and east sides of the map.
            let x = if owner == 0 {
                Fx::ratio(1, 5)
            } else {
                Fx::ratio(4, 5)
            };
            let start = FxVec2::new(map * x, map * Fx::ratio(1, 2));
            for i in 0..UNITS_PER_PLAYER {
                units.push(Unit {
                    id: UnitId(units.len() as u32),
                    owner,
                    pos: start + formation_offset(i, UNITS_PER_PLAYER),
                    target: None,
                });
            }
        }
        Sim {
            tick: 0,
            terrain,
            units,
        }
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn terrain(&self) -> &Terrain {
        &self.terrain
    }

    pub fn units(&self) -> &[Unit] {
        &self.units
    }

    pub fn unit(&self, id: UnitId) -> Option<&Unit> {
        let index = self.units.binary_search_by_key(&id, |u| u.id).ok()?;
        Some(&self.units[index])
    }

    /// Applies a command issued by `player`. Units that `player` does not own are
    /// ignored, so a buggy or malicious client cannot move someone else's army.
    pub fn apply(&mut self, player: PlayerId, command: &Command) {
        match command {
            Command::Move { units, target } => {
                let indices = self.owned_indices(player, units);
                for (i, &index) in indices.iter().enumerate() {
                    let dest = self
                        .terrain
                        .clamp(*target + formation_offset(i, indices.len()));
                    self.units[index].target = Some(dest);
                }
            }
            Command::Stop { units } => {
                for index in self.owned_indices(player, units) {
                    self.units[index].target = None;
                }
            }
        }
    }

    /// Advances the simulation by one tick.
    pub fn step(&mut self) {
        for unit in &mut self.units {
            let Some(target) = unit.target else { continue };
            let delta = target - unit.pos;
            let distance = delta.length();
            if distance <= UNIT_SPEED {
                unit.pos = target;
                unit.target = None;
            } else {
                unit.pos = unit.pos + delta * (UNIT_SPEED / distance);
            }
        }
        self.tick += 1;
    }

    /// Hash of the simulation state. Clients compare it to detect desyncs.
    pub fn checksum(&self) -> u64 {
        let mut hash = Fnv::default();
        hash.write(self.tick);
        for unit in &self.units {
            hash.write(unit.id.0 as u64);
            hash.write(unit.owner as u64);
            hash.write(unit.pos.x.raw() as u64);
            hash.write(unit.pos.z.raw() as u64);
            match unit.target {
                None => hash.write(0),
                Some(target) => {
                    hash.write(1);
                    hash.write(target.x.raw() as u64);
                    hash.write(target.z.raw() as u64);
                }
            }
        }
        hash.0
    }

    fn owned_indices(&self, player: PlayerId, ids: &[UnitId]) -> Vec<usize> {
        ids.iter()
            .filter_map(|&id| self.units.binary_search_by_key(&id, |u| u.id).ok())
            .filter(|&index| self.units[index].owner == player)
            .collect()
    }
}

/// Offset of the `i`-th of `n` units in a square grid centered on the origin.
fn formation_offset(i: usize, n: usize) -> FxVec2 {
    let mut cols = n.isqrt();
    if cols * cols < n {
        cols += 1;
    }
    let rows = n.div_ceil(cols);
    let centered = |k: usize, count: usize| {
        (Fx::from_int(k as i32) - Fx::ratio(count as i32 - 1, 2)) * UNIT_SPACING
    };
    FxVec2::new(centered(i % cols, cols), centered(i / cols, rows))
}

/// FNV-1a: stable across platforms and Rust versions, unlike `DefaultHasher`.
struct Fnv(u64);

impl Default for Fnv {
    fn default() -> Fnv {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
}

impl Fnv {
    fn write(&mut self, value: u64) {
        for byte in value.to_le_bytes() {
            self.0 = (self.0 ^ byte as u64).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn own_units(sim: &Sim, player: PlayerId) -> Vec<UnitId> {
        sim.units()
            .iter()
            .filter(|u| u.owner == player)
            .map(|u| u.id)
            .collect()
    }

    fn point(x: i32, z: i32) -> FxVec2 {
        FxVec2::new(Fx::from_int(x), Fx::from_int(z))
    }

    #[test]
    fn move_reaches_target_and_stops() {
        let mut sim = Sim::new(1, 2);
        let id = own_units(&sim, 0)[0];
        let target = point(100, 120);
        sim.apply(
            0,
            &Command::Move {
                units: vec![id],
                target,
            },
        );
        for _ in 0..TICK_RATE * 60 {
            sim.step();
        }
        let unit = sim.unit(id).unwrap();
        assert_eq!(unit.pos, target);
        assert_eq!(unit.target, None);
    }

    #[test]
    fn group_move_spreads_units_around_target() {
        let mut sim = Sim::new(1, 2);
        let ids = own_units(&sim, 1);
        let target = point(128, 128);
        sim.apply(
            1,
            &Command::Move {
                units: ids.clone(),
                target,
            },
        );
        let dests: Vec<FxVec2> = ids
            .iter()
            .map(|&id| sim.unit(id).unwrap().target.unwrap())
            .collect();
        for (i, a) in dests.iter().enumerate() {
            assert!(
                dests[i + 1..].iter().all(|b| b != a),
                "two units share a slot"
            );
        }
        let sum = dests
            .iter()
            .fold(FxVec2::default(), |acc, &d| acc + (d - target));
        assert_eq!(
            sum,
            FxVec2::default(),
            "formation is not centered on the target"
        );
    }

    #[test]
    fn players_cannot_command_enemy_units() {
        let mut sim = Sim::new(1, 2);
        let enemy = own_units(&sim, 0);
        sim.apply(
            1,
            &Command::Move {
                units: enemy.clone(),
                target: point(10, 10),
            },
        );
        assert!(
            enemy
                .iter()
                .all(|&id| sim.unit(id).unwrap().target.is_none())
        );
    }

    #[test]
    fn stop_cancels_move() {
        let mut sim = Sim::new(1, 1);
        let units = own_units(&sim, 0);
        sim.apply(
            0,
            &Command::Move {
                units: units.clone(),
                target: point(10, 10),
            },
        );
        sim.apply(
            0,
            &Command::Stop {
                units: units.clone(),
            },
        );
        assert!(
            units
                .iter()
                .all(|&id| sim.unit(id).unwrap().target.is_none())
        );
    }

    #[test]
    fn same_inputs_give_same_state() {
        let run = |target: FxVec2| {
            let mut sim = Sim::new(42, 2);
            let units = own_units(&sim, 0);
            sim.apply(0, &Command::Move { units, target });
            for _ in 0..100 {
                sim.step();
            }
            sim.checksum()
        };
        assert_eq!(run(point(50, 60)), run(point(50, 60)));
        assert_ne!(run(point(50, 60)), run(point(50, 61)));
    }
}
