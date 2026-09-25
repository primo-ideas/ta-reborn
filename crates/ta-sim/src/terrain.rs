use crate::{Fx, FxVec2};

/// Height-field of the map, generated from the match seed.
pub struct Terrain {
    /// Cells per side (the map is square).
    size: u32,
    /// `(size + 1)²` vertex heights, row by row along +z.
    heights: Vec<Fx>,
}

impl Terrain {
    /// World-space length of a cell side.
    pub const CELL_SIZE: Fx = Fx::from_int(2);

    pub fn generate(seed: u64, size: u32) -> Terrain {
        let heights = (0..=size)
            .flat_map(|z| (0..=size).map(move |x| hills(seed, x, z)))
            .collect();
        Terrain { size, heights }
    }

    /// Cells per side.
    pub fn size(&self) -> u32 {
        self.size
    }

    /// World-space length of a map side.
    pub fn world_size(&self) -> Fx {
        Fx::from_int(self.size as i32) * Self::CELL_SIZE
    }

    /// Height of grid vertex `(x, z)`, for `x, z` in `0..=size`.
    pub fn vertex_height(&self, x: u32, z: u32) -> Fx {
        self.heights[(z * (self.size + 1) + x) as usize]
    }

    /// Height at a world position, interpolated between vertices. Clamped to the map.
    pub fn height_at(&self, p: FxVec2) -> Fx {
        let p = self.clamp(p);
        let (gx, gz) = (p.x / Self::CELL_SIZE, p.z / Self::CELL_SIZE);
        // On the far edges, use the last cell (with t == 1).
        let last = self.size as i32 - 1;
        let (x, z) = (gx.floor().min(last), gz.floor().min(last));
        let (tx, tz) = (gx - Fx::from_int(x), gz - Fx::from_int(z));
        let (x, z) = (x as u32, z as u32);
        let near = lerp(self.vertex_height(x, z), self.vertex_height(x + 1, z), tx);
        let far = lerp(
            self.vertex_height(x, z + 1),
            self.vertex_height(x + 1, z + 1),
            tx,
        );
        lerp(near, far, tz)
    }

    /// Clamps a world position to the map bounds.
    pub fn clamp(&self, p: FxVec2) -> FxVec2 {
        let max = self.world_size();
        FxVec2::new(p.x.clamp(Fx::ZERO, max), p.z.clamp(Fx::ZERO, max))
    }
}

/// Rolling hills: octaves of value noise, shifted so that the lowest areas dip under
/// height 0 (the water level drawn by the client).
fn hills(seed: u64, x: u32, z: u32) -> Fx {
    // (lattice spacing in vertices, amplitude)
    const OCTAVES: [(u32, Fx); 4] = [
        (32, Fx::from_int(20)),
        (16, Fx::from_int(10)),
        (8, Fx::from_int(4)),
        (4, Fx::from_int(2)),
    ];
    let mut height = Fx::from_int(-12);
    for (octave, (spacing, amplitude)) in OCTAVES.into_iter().enumerate() {
        let octave_seed = seed.wrapping_add((octave as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        height += value_noise(octave_seed, x, z, spacing) * amplitude;
    }
    height
}

/// Random values in `[0, 1)` on a lattice of `spacing` vertices, smoothly interpolated.
fn value_noise(seed: u64, x: u32, z: u32, spacing: u32) -> Fx {
    let (cx, cz) = (x / spacing, z / spacing);
    let tx = smoothstep(Fx::ratio((x % spacing) as i32, spacing as i32));
    let tz = smoothstep(Fx::ratio((z % spacing) as i32, spacing as i32));
    let near = lerp(lattice(seed, cx, cz), lattice(seed, cx + 1, cz), tx);
    let far = lerp(lattice(seed, cx, cz + 1), lattice(seed, cx + 1, cz + 1), tx);
    lerp(near, far, tz)
}

/// Pseudo-random value in `[0, 1)` for a lattice point (SplitMix64 finalizer).
fn lattice(seed: u64, x: u32, z: u32) -> Fx {
    let mut h = seed ^ ((x as u64) << 32 | z as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    h = (h ^ (h >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    // The top 32 bits become the fractional part.
    Fx::from_raw((h >> 32) as i64)
}

fn smoothstep(t: Fx) -> Fx {
    t * t * (Fx::from_int(3) - Fx::from_int(2) * t)
}

fn lerp(a: Fx, b: Fx, t: Fx) -> Fx {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_depends_only_on_seed() {
        let a = Terrain::generate(7, 32);
        assert_eq!(a.heights, Terrain::generate(7, 32).heights);
        assert_ne!(a.heights, Terrain::generate(8, 32).heights);
    }

    #[test]
    fn height_at_matches_vertices_and_clamps() {
        let t = Terrain::generate(3, 16);
        let at = |x: i32, z: i32| t.height_at(FxVec2::new(Fx::from_int(x), Fx::from_int(z)));
        assert_eq!(at(4, 6), t.vertex_height(2, 3));
        assert_eq!(at(32, 32), t.vertex_height(16, 16));
        assert_eq!(at(-10, 1000), t.vertex_height(0, 16));
    }
}
