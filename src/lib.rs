mod assets;
mod bp;
mod error;
mod noise;
mod sampler;
mod samples;
mod structs;

use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::samples::WeightedSample;
use crate::structs::BaseRules;
use sns_asset_manager::prelude::{AssetBundle, DeclareTag, Registry};
use sns_helper::prelude::{NoiseSampler, SimdNoiseBackend};
use structs::BPSettings;

pub use crate::assets::*;
pub use crate::bp::{
    WorldPlate, macro_bp::MacroPipeline, meso_bp::MesoPlate, terrain_bp::TerrainPlate,
    zone_bp::ZonePlate,
};
pub use crate::error::BaseplateError;
pub use crate::noise::NoiseScaleConfig;
pub use crate::sampler::GridSampleCache;
pub use crate::samples::{MacroSample, MicroSample};
pub use crate::structs::{FeatureContribution, FeatureSample};
/*
 * Macro: Defines the large-scale geography and environmental identity of the world. (broad climate, large noise,landmass traits)
 * Meso:  Defines the ecological composition and variation within macro areas. (Biomes/sub biomes, medium noise)
 * Micro: Defines local terrain variation and fine-scale procedural characteristics. (Terrain, local noise, small feature placements )
 * Samples: Output of all three scales, as well as biome/zone/sub biome etc.
 */

pub mod prelude {
    pub use crate::assets::*;
    pub use crate::structs::*;
    pub use crate::{
        BaseplateError, GridSampleCache, MacroPipeline, MacroSample, MesoPlate, MicroSample,
        NoiseScaleConfig, TerrainPlate, WorldPlate, ZonePlate,
    };
}

pub fn load_baseplates<'a>(
    registry: &'a mut Registry,
    bp_bundle: &AssetBundle,
) -> Result<WorldPlate<'a>, BaseplateError> {
    let bp_settings_val = bp_bundle
        .settings
        .clone()
        .ok_or_else(|| BaseplateError::AssetLoad("baseplate settings not found".into()))?;

    let bp_settings: BPSettings = bp_settings_val
        .into_rust()
        .map_err(|_| BaseplateError::AssetLoad("failed to convert baseplate settings".into()))?;

    let mut issues = Vec::new();
    bp_settings.validate(&mut issues);
    if !issues.is_empty() {
        return Err(BaseplateError::InvalidConfiguration(issues));
    }

    let bp_tag = registry
        .lookup_string::<DeclareTag>("sns.bp.declare:bp_tag".to_string())
        .ok_or_else(|| BaseplateError::AssetLoad("failed to load sns.bp.declare:bp_tag".into()))?;

    let noise_contri = bp_tag.items::<NoiseContribution>(registry).map_err(|err| {
        BaseplateError::AssetLoad(format!("failed to load NoiseContribution items: {err}"))
    })?;
    if noise_contri.len() != 3 {
        return Err(BaseplateError::AssetLoad(
            "failed to load NoiseContribution".into(),
        ));
    }

    let zone_map = bp_tag.map::<ZoneArchetype>(registry).map_err(|err| {
        BaseplateError::AssetLoad(format!("failed to load ZoneArchetype map: {err}"))
    })?;
    let enviro_map = bp_tag.map::<EnvironmentField>(registry).map_err(|err| {
        BaseplateError::AssetLoad(format!("failed to load EnvironmentField map: {err}"))
    })?;

    let biome_map = bp_tag
        .map::<Biome>(registry)
        .map_err(|err| BaseplateError::AssetLoad(format!("failed to load Biome map: {err}")))?;
    let subbiome_map = bp_tag
        .map::<SubBiome>(registry)
        .map_err(|err| BaseplateError::AssetLoad(format!("failed to load SubBiome map: {err}")))?;

    let terrain_map = bp_tag.map::<TerrainProfile>(registry).map_err(|err| {
        BaseplateError::AssetLoad(format!("failed to load TerrainProfile map: {err}"))
    })?;

    let mut noise_map = BTreeMap::new();

    for item in bp_tag.items::<NoiseField>(registry).map_err(|err| {
        BaseplateError::AssetLoad(format!("failed to load NoiseField items: {err}"))
    })? {
        noise_map.insert(
            item.get_val(),
            NoiseSampler::<SimdNoiseBackend>::from(item.get()),
        );
    }

    let zone_id = bp_tag.items::<ZoneArchetype>(registry).map_err(|err| {
        BaseplateError::AssetLoad(format!("failed to load ZoneArchetype items: {err}"))
    })?;

    let noise_scale = NoiseScaleConfig::new(
        noise_contri[0].get(),
        noise_contri[1].get(),
        noise_contri[2].get(),
    );

    let zone_plate = ZonePlate::new(zone_map, zone_id, bp_settings.zone);

    let meso_plate = MesoPlate::new(biome_map, subbiome_map, bp_settings.biome.clone());

    let terrain_plate = TerrainPlate::new(terrain_map, bp_settings.biome);

    let base_rule_map = bp_tag
        .map::<BaseRule>(registry)
        .map_err(|err| BaseplateError::AssetLoad(format!("failed to load BaseRule map: {err}")))?;
    let feature_rule_map = bp_tag.map::<FeatureRule>(registry).map_err(|err| {
        BaseplateError::AssetLoad(format!("failed to load FeatureRule map: {err}"))
    })?;

    let base_rules = BaseRules::new(base_rule_map, feature_rule_map);

    WorldPlate::try_new(
        noise_scale,
        zone_plate,
        meso_plate,
        terrain_plate,
        noise_map,
        enviro_map,
        base_rules,
        bp_settings.grid,
    )
}

pub(crate) fn merge_lerp<T: WeightedSample>(a: &[T], b: &[T], t: f32) -> Vec<T> {
    let mut out = Vec::with_capacity(a.len() + b.len());

    let mut i = 0;
    let mut j = 0;

    while i < a.len() && j < b.len() {
        match a[i].id().cmp(&b[j].id()) {
            Ordering::Equal => {
                let item = a[i].lerp(&b[j], t);
                if item.weight() > f32::EPSILON {
                    out.push(item);
                }
                i += 1;
                j += 1;
            }
            Ordering::Less => {
                let item = a[i].scaled(a[i].weight() * (1.0 - t));
                if item.weight() > f32::EPSILON {
                    out.push(item);
                }
                i += 1;
            }
            Ordering::Greater => {
                let item = b[j].scaled(b[j].weight() * t);
                if item.weight() > f32::EPSILON {
                    out.push(item);
                }
                j += 1;
            }
        }
    }

    while i < a.len() {
        let item = a[i].scaled(a[i].weight() * (1.0 - t));
        if item.weight() > f32::EPSILON {
            out.push(item);
        }
        i += 1;
    }

    while j < b.len() {
        let item = b[j].scaled(b[j].weight() * t);
        if item.weight() > f32::EPSILON {
            out.push(item);
        }
        j += 1;
    }

    out
}
