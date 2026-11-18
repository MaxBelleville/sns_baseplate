use sns_asset_manager::prelude::*;
use sns_helper::prelude::{FieldMath, FieldValidate};

use super::structs::{DensityPoint, PartialEnviroField};

#[derive(Default, MakeAsset, Clone)]
pub struct NoiseContribution {
    noise_id: AssetIdVal<NoiseField>,
    environment_id: AssetIdVal<EnvironmentField>,
}
#[derive(Default, MakeAsset, FieldValidate)]
#[constraint(is_finite, min = 0.0)]
pub struct NoiseField {
    #[copy_getter]
    frequency: f32,
    #[copy_getter]
    octaves: u8,
    #[copy_getter]
    gain: f32,
    #[copy_getter]
    lacunarity: f32,
    #[copy_getter]
    mult: f32,
}

#[derive(Default, MakeAsset, Clone)]
pub struct ZoneArchetype {
    bias_id: AssetIdVal<EnvironmentField>,
    max_delta_id: Option<AssetIdVal<EnvironmentField>>,
    base_rule_ids: Vec<AssetIdVal<BaseRule>>,
    feature_rule_ids: Vec<AssetIdVal<FeatureRule>>,
}

#[derive(Default, MakeAsset)]
pub struct FeatureRule {
    // feature_id: AssetIdVal<FeatureDefinition>,
    // #[copy_getter]
    // chance_override: Option<f32>,
    // soft_avoid_subbiome_ids: Vec<AssetIdVal<SubBiome>>,
}

#[derive(Default, MakeAsset)]
pub struct Biome {
    center: PartialEnviroField,
    width: PartialEnviroField,
    sub_ids: Vec<AssetIdVal<SubBiome>>,
    terrain_profile_id: AssetIdVal<TerrainProfile>,
    base_rule_ids: Vec<AssetIdVal<BaseRule>>,
    feature_rule_ids: Vec<AssetIdVal<FeatureRule>>,
}
#[derive(Default, MakeAsset, FieldValidate)]
#[constraint(is_finite, min = 0.0)]
pub struct SubBiome {
    #[skip_field]
    parent_id: AssetIdVal<Biome>,
    #[copy_getter]
    threshold: f32,
    #[copy_getter]
    weight: f32,
    #[skip_field]
    terrain_profile_id: Option<AssetIdVal<TerrainProfile>>,
    #[skip_field]
    base_rule_ids: Vec<AssetIdVal<BaseRule>>,
    #[skip_field]
    feature_rule_ids: Vec<AssetIdVal<FeatureRule>>,
}

#[derive(Default, MakeAsset)]

pub struct BaseRule {
    // #[copy_getter]
    // min_weight: f32,
    // #[copy_getter]
    // weight: Option<f32>,
    // #[copy_getter]
    // is_feature: bool,
}

#[derive(Default, Copy, Clone, FieldMath, FieldValidate, MakeAsset)]
#[constraint(is_finite)]
pub struct EnvironmentField {
    #[copy_getter]
    temperature: f32,
    #[copy_getter]
    moisture: f32,
    #[copy_getter]
    continentalness: f32,
    #[copy_getter]
    ruggedness: f32,
}
impl std::ops::Mul<f32> for EnvironmentField {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self {
        Self {
            temperature: self.temperature * rhs,
            moisture: self.moisture * rhs,
            continentalness: self.continentalness * rhs,
            ruggedness: self.ruggedness * rhs,
        }
    }
}
impl EnvironmentField {
    pub fn abs_rugedness(&mut self) {
        self.ruggedness = self.ruggedness.abs();
    }
}

#[derive(Default, MakeAsset, FieldValidate)]
#[constraint(is_finite, min = 0.0)]
pub struct TerrainProfile {
    #[skip_field]
    base_noise: AssetIdVal<NoiseField>,
    #[copy_getter]
    height_offset: f32, // mean elevation contribution
    #[copy_getter]
    height_variance: f32, // amplitude of deviation
    #[copy_getter]
    roughness: f32,
    #[copy_getter]
    erosion_strength: f32,
    #[skip_field]
    density_curve_id: Option<AssetIdVal<DensityCurve>>,
}

#[derive(Default, MakeAsset)]
pub struct DensityCurve {
    points: Vec<DensityPoint>,
}
