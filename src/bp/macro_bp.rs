use std::collections::BTreeMap;

use super::meso_bp::MesoPlate;

use super::zone_bp::ZonePlate;

use crate::BaseplateError;
use crate::noise::{FieldGenerator, NoiseScaleConfig};
use crate::sampler::*;
use crate::samples::MacroSample;

use crate::assets::{EnvironmentField, NoiseField, TerrainProfile};
use crate::structs::{BaseRules, PartialEnviroField};
use sns_asset_manager::prelude::*;

use sns_helper::prelude::{IVec2, NoiseSampler, SimdNoiseBackend, Vec2};

#[derive(Default)]
pub struct MacroPlate<'a> {
    pipeline: MacroPipeline<'a>,
    cache: GridSampleCache<MacroSample>,
}

impl<'a> MacroPlate<'a> {
    pub fn new(pipeline: MacroPipeline<'a>, grid: Vec2) -> Self {
        Self {
            pipeline,
            cache: GridSampleCache::new(grid.x as i32, grid.y),
        }
    }

    pub fn macro_update(
        &mut self,
        world_pos: Vec2,
        seed: i32,
        enviro_fields: &BTreeMap<AssetIdVal<EnvironmentField>, &'a EnvironmentField>,
        noise_fields: &BTreeMap<AssetIdVal<NoiseField>, NoiseSampler<SimdNoiseBackend>>,
        base_rules: &BaseRules<'a>,
    ) -> Result<(), BaseplateError> {
        let mut sampler = MacroGridSampler::new(
            &self.pipeline,
            self.cache.grid_size(),
            enviro_fields,
            noise_fields,
            base_rules,
        );

        self.cache.update(&mut sampler, world_pos, seed)
    }

    pub(crate) fn get_cache(&self) -> &GridSampleCache<MacroSample> {
        &self.cache
    }
}
#[derive(Default)]
pub struct MacroPipeline<'a> {
    field_gen: FieldGenerator,
    zone_plate: ZonePlate<'a>,
    meso_plate: MesoPlate<'a>,
}

impl<'a> MacroPipeline<'a> {
    pub fn new(config: NoiseScaleConfig, zone: ZonePlate<'a>, meso: MesoPlate<'a>) -> Self {
        Self {
            field_gen: FieldGenerator::new(config),
            zone_plate: zone,
            meso_plate: meso,
        }
    }

    pub(crate) fn validate(
        &self,
        terrain_profiles: &BTreeMap<AssetIdVal<TerrainProfile>, &'a TerrainProfile>,
        enviro_fields: &BTreeMap<AssetIdVal<EnvironmentField>, &'a EnvironmentField>,
        noise_fields: &BTreeMap<AssetIdVal<NoiseField>, NoiseSampler<SimdNoiseBackend>>,
        issues: &mut Vec<String>,
    ) {
        self.field_gen.validate(enviro_fields, noise_fields, issues);
        self.zone_plate.validate(enviro_fields, issues);
        self.meso_plate.validate(terrain_profiles, issues);
    }

    pub fn sample(
        &self,
        coord: Vec2,
        seed: i32,
        enviro_fields: &BTreeMap<AssetIdVal<EnvironmentField>, &'a EnvironmentField>,
        noise_fields: &BTreeMap<AssetIdVal<NoiseField>, NoiseSampler<SimdNoiseBackend>>,
    ) -> Result<MacroSample, BaseplateError> {
        if !coord.is_finite() {
            return Err(BaseplateError::InvalidSamplePosition);
        }
        let mut biased_field = self
            .field_gen
            .sample(coord, seed, enviro_fields, noise_fields)?;

        let meso_sample = self.zone_plate.sample(coord, seed)?;
        let zone_arch_id = meso_sample.get_archetype_id();
        let zone_arch = self.zone_plate.get_archetype(zone_arch_id).ok_or_else(|| {
            BaseplateError::MissingAsset(format!("zone archetype {}", zone_arch_id.get()))
        })?;

        let zone_bias = enviro_fields
            .get(zone_arch.bias_id())
            .cloned()
            .ok_or_else(|| {
                BaseplateError::MissingAsset(format!(
                    "environment field {}",
                    zone_arch.bias_id().get()
                ))
            })?;

        biased_field = biased_field + *zone_bias * meso_sample.get_edge_factor();
        if biased_field.validate().is_err() {
            return Err(BaseplateError::NonFiniteSample("biased environment field"));
        }

        let micro_noise_id = self.field_gen.micro_noise_id();
        let micro_noise = noise_fields.get(micro_noise_id).ok_or_else(|| {
            BaseplateError::MissingAsset(format!("noise field {}", micro_noise_id.get()))
        })?;

        let biomes = self.meso_plate.sample(
            PartialEnviroField::from(biased_field),
            coord,
            seed,
            micro_noise,
        )?;

        Ok(MacroSample::new(
            biased_field,
            biomes,
            zone_arch_id,
            meso_sample.get_edge_factor(),
            zone_arch.base_rule_ids().clone(),
            zone_arch.feature_rule_ids().clone(),
        ))
    }
}

pub struct MacroGridSampler<'a> {
    pipeline: &'a MacroPipeline<'a>,
    enviro_fields: &'a BTreeMap<AssetIdVal<EnvironmentField>, &'a EnvironmentField>,
    noise_fields: &'a BTreeMap<AssetIdVal<NoiseField>, NoiseSampler<SimdNoiseBackend>>,
    base_rules: &'a BaseRules<'a>,
    grid_size: f32,
}
impl<'a> MacroGridSampler<'a> {
    pub(crate) fn new(
        pipeline: &'a MacroPipeline,
        grid_size: f32,
        enviro_fields: &'a BTreeMap<AssetIdVal<EnvironmentField>, &'a EnvironmentField>,
        noise_fields: &'a BTreeMap<AssetIdVal<NoiseField>, NoiseSampler<SimdNoiseBackend>>,
        base_rules: &'a BaseRules,
    ) -> Self {
        Self {
            pipeline,
            enviro_fields,
            noise_fields,
            base_rules,
            grid_size,
        }
    }
}

impl<'a> GridSampler<MacroSample> for MacroGridSampler<'a> {
    fn sample_cell(&mut self, grid_coord: IVec2, seed: i32) -> Result<MacroSample, BaseplateError> {
        let world_pos = Vec2::new(
            grid_coord.x as f32 * self.grid_size,
            grid_coord.y as f32 * self.grid_size,
        );

        let mut samples =
            self.pipeline
                .sample(world_pos, seed, self.enviro_fields, self.noise_fields)?;

        let zone_rules = samples.base_rule_ids().clone();
        //self.base_rules.evaluate(&mut samples, &zone_rules);
        Ok(samples)
    }
}
