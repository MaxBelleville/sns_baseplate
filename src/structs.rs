use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sns_asset_manager::prelude::*;

use super::assets::{BaseRule, EnvironmentField, FeatureRule};
use sns_helper::{
    IterableField,
    prelude::{FieldMath, IVec2, IterableFields, Vec2, check_f32_values, check_vec, check_vec_opt},
};

#[derive(Debug, Default, Deserialize)]
pub(crate) struct BPSettings {
    pub(crate) biome: BiomeSettings,
    pub(crate) zone: ZoneSettings,
    pub(crate) grid: GridSettings,
}
impl BPSettings {
    pub(crate) fn validate(&self, issues: &mut Vec<String>) {
        self.biome.validate("settings.biome", issues);
        self.grid.validate("settings.grid", issues);
        if !self.zone.zone_size.is_finite() || self.zone.zone_size <= 0.0 {
            issues.push("settings.zone.zone_size must be finite and greater than zero".into());
        }
    }
}
#[derive(Debug, Default, Deserialize, Clone)]
pub struct BiomeSettings {
    pub biome_base_weight: f32,
    pub subbiome_base_weight: f32,
    pub subbiome_offset_scale: f32,
}
impl BiomeSettings {
    pub(crate) fn validate(&self, path: &str, issues: &mut Vec<String>) {
        check_vec_opt::<f32>(
            issues,
            &vec![self.biome_base_weight, self.subbiome_base_weight],
            |value| !value.is_finite() || *value < 0.0,
            Some(!self.subbiome_offset_scale.is_finite()),
            format!("{path} must be finite and nonnegative(for weights)"),
        );
    }
}
#[derive(Debug, Default, Deserialize)]
pub struct GridSettings {
    pub micro_size: Vec2,
    pub macro_size: Vec2,
}
impl GridSettings {
    pub(crate) fn validate(&self, path: &str, issues: &mut Vec<String>) {
        check_vec::<f32>(
            issues,
            &vec![self.micro_size.x, self.macro_size.x],
            |value| !value.is_finite() || *value < 0.0,
            format!("{path} cache resolutions must be finite and nonnegative"),
        );
        check_vec::<f32>(
            issues,
            &vec![self.micro_size.y, self.macro_size.y],
            |value| !value.is_finite() || *value <= 0.0,
            format!("{path} cache grid sizes must be finite and greater than zero"),
        );
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct ZoneSettings {
    pub zone_size: f32,
}

#[derive(Default)]
pub(crate) struct VoronoiResult {
    pub(crate) closest: IVec2,
    pub(crate) d1: f32,
    pub(crate) d2: f32,
}

#[derive(Clone)]
pub struct FeatureContribution {
    //   pub feature_id: AssetIdVal<FeatureDefinition>,
    pub rule_id: AssetIdVal<BaseRule>,
    pub source_id: u32,
    pub parent_source_id: Option<u32>,
    pub source_weight: f32,
    pub support_weight: f32,
    pub chance: f32,
}

#[derive(Clone)]
pub struct FeatureSample {
    //   pub feature_id: AssetIdVal<FeatureDefinition>,
    pub chance: f32,
    pub contributions: Vec<FeatureContribution>,
}
impl FeatureSample {
    pub fn probability(&self) -> f32 {
        self.chance
    }
}

#[derive(Default, Copy, Clone, FieldMath, IterableFields, Deserialize, Serialize)]

pub struct PartialEnviroField {
    temperature: f32,
    moisture: f32,
}
impl PartialEnviroField {
    pub fn new(temperature: f32, moisture: f32) -> Self {
        Self {
            temperature,
            moisture,
        }
    }
    pub fn get_temperature(&self) -> f32 {
        self.temperature
    }
    pub fn get_moisture(&self) -> f32 {
        self.moisture
    }
    pub(crate) fn validate(&self, path: &str, issues: &mut Vec<String>) {
        check_f32_values(
            issues,
            &IterableField::<f32>::iter(self).copied().collect(),
            format!("{path} must contain only finite values"),
        );
    }
}
impl From<EnvironmentField> for PartialEnviroField {
    fn from(field: EnvironmentField) -> Self {
        Self {
            temperature: field.temperature().clone(),
            moisture: field.moisture().clone(),
        }
    }
}

#[derive(Default, Deserialize, Serialize, Clone)]

pub struct DensityPoint {
    height: f32,
    density: f32,
}
impl DensityPoint {
    pub fn get_height(&self) -> f32 {
        self.height
    }
    pub fn get_density(&self) -> f32 {
        self.density
    }
}

#[derive(Default)]
pub struct BaseRules<'a> {
    rules: BTreeMap<AssetIdVal<BaseRule>, &'a BaseRule>,
    feature_rules: BTreeMap<AssetIdVal<FeatureRule>, &'a FeatureRule>,
    // features: BTreeMap<AssetIdVal<FeatureDefinition>, FeatureDefinition>,
}
impl<'a> BaseRules<'a> {
    pub fn new(
        rules: BTreeMap<AssetIdVal<BaseRule>, &'a BaseRule>,
        feature_rules: BTreeMap<AssetIdVal<FeatureRule>, &'a FeatureRule>,
    ) -> Self {
        Self {
            rules,
            feature_rules, //  features: BTreeMap::new(),
        }
    }
    pub fn with_feature_assets(
        mut self,
        feature_rules: BTreeMap<AssetIdVal<FeatureRule>, &'a FeatureRule>,
        // features: BTreeMap<AssetIdVal<FeatureDefinition>, FeatureDefinition>,
    ) -> Self {
        self.feature_rules = feature_rules;
        //   self.features = features;
        self
    }
    pub fn rules(&self) -> &BTreeMap<AssetIdVal<BaseRule>, &BaseRule> {
        &self.rules
    }
    pub fn rule(&self, id: AssetIdVal<BaseRule>) -> Option<&BaseRule> {
        self.rules.get(&id).cloned()
    }
}

fn valid_chance(value: f32) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}
