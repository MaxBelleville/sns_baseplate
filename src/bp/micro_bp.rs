use std::collections::BTreeMap;

use super::terrain_bp::TerrainPlate;

use crate::BaseplateError;
use crate::assets::NoiseField;
use crate::sampler::*;
use crate::samples::{MacroSample, MicroSample};
use sns_asset_manager::prelude::*;

use sns_helper::prelude::{IVec2, NoiseSampler, SimdNoiseBackend, Vec2, hash_vec2};

#[derive(Default)]
pub struct MicroPlate {
    cache: GridSampleCache<MicroSample>,
}

impl MicroPlate {
    pub fn new(grid: Vec2) -> Self {
        Self {
            cache: GridSampleCache::new(grid.x as i32, grid.y),
        }
    }
    pub(crate) fn cache(&self) -> &GridSampleCache<MicroSample> {
        &self.cache
    }
    pub fn micro_update(
        &mut self,
        world_pos: Vec2,
        seed: i32,
        macro_cache: &GridSampleCache<MacroSample>,
        terrain_plate: &TerrainPlate,
        noise_fields: &BTreeMap<AssetIdVal<NoiseField>, NoiseSampler<SimdNoiseBackend>>,
    ) -> Result<(), BaseplateError> {
        let mut sampler = MicroGridSampler::new(
            macro_cache,
            terrain_plate,
            noise_fields,
            self.cache.grid_size(),
        );

        self.cache.update(&mut sampler, world_pos, seed)
    }
}

pub struct MicroGridSampler<'a> {
    macro_cache: &'a GridSampleCache<MacroSample>,
    terrain_plate: &'a TerrainPlate<'a>,
    noise_fields: &'a BTreeMap<AssetIdVal<NoiseField>, NoiseSampler<SimdNoiseBackend>>,
    grid_size: f32,
}
impl<'a> MicroGridSampler<'a> {
    fn interpolate(&self, world_pos: Vec2) -> Result<MacroSample, BaseplateError> {
        self.macro_cache
            .interpolate(world_pos, |a, b, t| a.interpolate(b, t))
    }

    pub(crate) fn new(
        macro_cache: &'a GridSampleCache<MacroSample>,
        terrain_plate: &'a TerrainPlate,
        noise_fields: &'a BTreeMap<AssetIdVal<NoiseField>, NoiseSampler<SimdNoiseBackend>>,
        grid_size: f32,
    ) -> Self {
        Self {
            macro_cache,
            terrain_plate,
            noise_fields,
            grid_size,
        }
    }
}

impl<'a> GridSampler<MicroSample> for MicroGridSampler<'a> {
    fn sample_cell(&mut self, grid_coord: IVec2, seed: i32) -> Result<MicroSample, BaseplateError> {
        let base_pos = grid_coord.as_vec2() * self.grid_size;

        let jitter = (hash_vec2(grid_coord, seed) - 0.5) * self.grid_size;

        let world_pos = base_pos + jitter;
        if !world_pos.is_finite() {
            return Err(BaseplateError::InvalidSamplePosition);
        }

        // interpolate ONLY from already-updated macro cache
        let macro_sample = self.interpolate(world_pos)?;

        self.terrain_plate
            .blend(world_pos, seed, macro_sample, self.noise_fields)
    }
}

#[allow(dead_code)]
fn select_zone_archetype<T: Copy>(left: T, left_edge: f32, right: T, right_edge: f32, t: f32) -> T {
    if select_zone_right(left, left_edge, right, right_edge, t) {
        right
    } else {
        left
    }
}

#[allow(dead_code)]
fn select_zone_right<T>(_left: T, left_edge: f32, _right: T, right_edge: f32, t: f32) -> bool {
    let left_confidence = (1.0 - t) * left_edge;
    let right_confidence = t * right_edge;

    if left_confidence > right_confidence {
        false
    } else if right_confidence > left_confidence || t >= 0.5 {
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::select_zone_archetype;

    #[test]
    fn zone_identity_uses_interpolation_weighted_edge_confidence() {
        assert_eq!(select_zone_archetype('L', 1.0, 'R', 0.25, 0.4), 'L');
        assert_eq!(select_zone_archetype('L', 0.25, 'R', 1.0, 0.4), 'R');
    }

    #[test]
    fn zone_identity_ties_choose_the_nearest_endpoint() {
        assert_eq!(select_zone_archetype('L', 1.0 / 3.0, 'R', 1.0, 0.25), 'L');
        assert_eq!(select_zone_archetype('L', 0.5, 'R', 0.5, 0.5), 'R');
    }
}
