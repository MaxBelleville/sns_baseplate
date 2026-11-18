use std::collections::BTreeMap;

pub mod macro_bp;
pub mod meso_bp;
pub mod micro_bp;
pub mod terrain_bp;
pub mod zone_bp;

use crate::BaseplateError;
use crate::noise::NoiseScaleConfig;
use crate::sampler::GridSampleCache;
use crate::samples::{MacroSample, MicroSample};

use macro_bp::{MacroPipeline, MacroPlate};
use meso_bp::MesoPlate;
use micro_bp::MicroPlate;
use sns_asset_manager::prelude::AssetIdVal;
use sns_helper::prelude::{NoiseSampler, SimdNoiseBackend, Vec2};
use terrain_bp::TerrainPlate;
use zone_bp::ZonePlate;

use crate::assets::{EnvironmentField, NoiseField};
use crate::structs::{BaseRules, GridSettings};

pub struct WorldPlate<'a> {
    macro_plate: MacroPlate<'a>,
    micro_plate: MicroPlate,
    terrain_plate: TerrainPlate<'a>,
    noise_fields: BTreeMap<AssetIdVal<NoiseField>, NoiseSampler<SimdNoiseBackend>>,
    enviro_fields: BTreeMap<AssetIdVal<EnvironmentField>, &'a EnvironmentField>,
    base_rules: BaseRules<'a>,
}

impl<'a> WorldPlate<'a> {
    /// Builds a baseplate after validating all definitions and cache settings.

    pub fn try_new(
        config: NoiseScaleConfig,
        zone_plate: ZonePlate<'a>,
        meso_plate: MesoPlate<'a>,
        terrain_plate: TerrainPlate<'a>,
        noise_fields: BTreeMap<AssetIdVal<NoiseField>, NoiseSampler<SimdNoiseBackend>>,
        enviro_fields: BTreeMap<AssetIdVal<EnvironmentField>, &'a EnvironmentField>,
        base_rules: BaseRules<'a>,
        grid_settings: GridSettings,
    ) -> Result<Self, BaseplateError> {
        let pipeline = MacroPipeline::new(config, zone_plate, meso_plate);
        let mut issues = Vec::new();

        pipeline.validate(
            terrain_plate.terrain_profiles(),
            &enviro_fields,
            &noise_fields,
            &mut issues,
        );

        terrain_plate.validate(&noise_fields, &mut issues);
        //base_rules.validate(&mut issues);

        validate_cache("macro", grid_settings.macro_size, &mut issues);

        if grid_settings.macro_size.x < 1.0 {
            issues.push(
                "macro cache resolution must be at least one for bilinear interpolation".into(),
            );
        }

        validate_cache("micro", grid_settings.micro_size, &mut issues);

        if !issues.is_empty() {
            return Err(BaseplateError::InvalidConfiguration(issues));
        }

        Ok(Self {
            macro_plate: MacroPlate::new(pipeline, grid_settings.macro_size),
            micro_plate: MicroPlate::new(grid_settings.micro_size),
            terrain_plate,
            noise_fields,
            enviro_fields,
            base_rules,
        })
    }

    pub fn macro_update(&mut self, world_pos: Vec2, seed: i32) -> Result<(), BaseplateError> {
        self.macro_plate.macro_update(
            world_pos,
            seed,
            &self.enviro_fields,
            &self.noise_fields,
            &self.base_rules,
        )
    }

    pub fn micro_update(&mut self, world_pos: Vec2, seed: i32) -> Result<(), BaseplateError> {
        self.micro_plate.micro_update(
            world_pos,
            seed,
            self.macro_plate.get_cache(),
            &self.terrain_plate,
            &self.noise_fields,
        )
    }

    pub fn macro_samples(&self) -> &GridSampleCache<MacroSample> {
        self.macro_plate.get_cache()
    }

    pub fn micro_samples(&self) -> &GridSampleCache<MicroSample> {
        self.micro_plate.cache()
    }

    /// Returns interpolated macro data and feature probabilities without generating terrain.
    pub fn macro_sample_at(&self, world_pos: Vec2) -> Result<MacroSample, BaseplateError> {
        self.macro_plate
            .get_cache()
            .interpolate(world_pos, |a, b, t| a.interpolate(b, t))
    }
}

fn validate_cache(name: &str, grid: Vec2, issues: &mut Vec<String>) {
    if !grid.x.is_finite() || grid.x < 0.0 {
        issues.push(format!("{name} cache cell resolution must be nonnegative"));
    }
    if !grid.y.is_finite() || grid.y <= 0.0 {
        issues.push(format!(
            "{name} cache grid size must be finite and greater than zero"
        ));
    }
}
