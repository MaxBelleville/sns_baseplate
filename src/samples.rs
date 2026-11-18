use super::merge_lerp;

use crate::BaseplateError;
use sns_asset_manager::prelude::*;
use sns_helper::IterableField;
use sns_helper::prelude::IterableFields;
use sns_helper::prelude::{IVec2, lerp};

use super::assets::{
    BaseRule, Biome, DensityCurve, EnvironmentField, FeatureRule, SubBiome, TerrainProfile,
    ZoneArchetype,
};
use super::structs::FeatureSample;

#[derive(Default, Clone)]
pub struct MacroSample {
    enviro: EnvironmentField,
    biome_samples: Vec<BiomeSample>,
    archetype_id: AssetIdVal<ZoneArchetype>,
    edge_factor: f32,
    base_rule_ids: Vec<AssetIdVal<BaseRule>>,
    feature_rule_ids: Vec<AssetIdVal<FeatureRule>>,
    feature_samples: Vec<FeatureSample>,
}

impl MacroSample {
    pub fn new(
        enviro: EnvironmentField,
        biome_samples: Vec<BiomeSample>,
        archetype_id: AssetIdVal<ZoneArchetype>,
        edge_factor: f32,
        base_rule_ids: Vec<AssetIdVal<BaseRule>>,
        feature_rule_ids: Vec<AssetIdVal<FeatureRule>>,
    ) -> Self {
        Self {
            enviro,
            biome_samples,
            archetype_id,
            edge_factor,
            base_rule_ids,
            feature_rule_ids,
            feature_samples: Vec::new(),
        }
    }

    pub fn environment(&self) -> EnvironmentField {
        self.enviro
    }
    pub fn set_environment(&mut self, field: EnvironmentField) {
        self.enviro = field;
    }

    pub fn biome_samples(&self) -> &Vec<BiomeSample> {
        &self.biome_samples
    }
    pub fn mut_biome_samples(&mut self) -> &mut Vec<BiomeSample> {
        &mut self.biome_samples
    }
    pub fn set_biomes_samples(&mut self, biome_samples: Vec<BiomeSample>) {
        self.biome_samples = biome_samples;
    }

    pub fn archetype_id(&self) -> AssetIdVal<ZoneArchetype> {
        self.archetype_id
    }

    pub fn edge_factor(&self) -> f32 {
        self.edge_factor
    }

    pub fn base_rule_ids(&self) -> &Vec<AssetIdVal<BaseRule>> {
        &self.base_rule_ids
    }
    pub fn feature_rule_ids(&self) -> &[AssetIdVal<FeatureRule>] {
        &self.feature_rule_ids
    }
    pub fn feature_samples(&self) -> &[FeatureSample] {
        &self.feature_samples
    }

    pub(crate) fn set_feature_samples(&mut self, value: Vec<FeatureSample>) {
        self.feature_samples = value;
    }

    /// Interpolates the baseplate outputs, including feature eligibility, without generating terrain.
    pub fn interpolate(&self, other: &Self, t: f32) -> Self {
        let mut out = Self::new(
            self.enviro.lerp(&other.enviro, t),
            merge_lerp(&self.biome_samples, &other.biome_samples, t),
            self.archetype_id,
            lerp(self.edge_factor, other.edge_factor, t),
            self.base_rule_ids.clone(),
            self.feature_rule_ids.clone(),
        );
        out.archetype_id = if select_zone_right(self.edge_factor, other.edge_factor, t) {
            other.archetype_id
        } else {
            self.archetype_id
        };
        out.base_rule_ids = if out.archetype_id == other.archetype_id {
            other.base_rule_ids.clone()
        } else {
            self.base_rule_ids.clone()
        };
        out.feature_rule_ids = if out.archetype_id == other.archetype_id {
            other.feature_rule_ids.clone()
        } else {
            self.feature_rule_ids.clone()
        };

        out
    }
}

fn select_zone_right(left_edge: f32, right_edge: f32, t: f32) -> bool {
    let left_confidence = (1.0 - t) * left_edge;
    let right_confidence = t * right_edge;
    right_confidence > left_confidence || (right_confidence == left_confidence && t >= 0.5)
}

#[derive(Default)]
pub struct MesoSample {
    _coord: IVec2,
    archetype_id: AssetIdVal<ZoneArchetype>,
    edge_factor: f32,
}
impl MesoSample {
    pub fn _get_coord(&self) -> IVec2 {
        self._coord
    }
    pub fn get_archetype_id(&self) -> AssetIdVal<ZoneArchetype> {
        self.archetype_id
    }
    pub fn get_edge_factor(&self) -> f32 {
        self.edge_factor
    }

    pub(crate) fn new(
        closest: IVec2,
        archetype_id: AssetIdVal<ZoneArchetype>,
        edge_factor: f32,
    ) -> Self {
        Self {
            _coord: closest,
            archetype_id,
            edge_factor,
        }
    }
}

#[derive(Default, IterableFields)]
pub struct MicroSample {
    #[skip_field]
    field: EnvironmentField,

    height_mean: f32,
    height_variance: f32,
    roughness: f32,
    erosion_bias: f32,
    #[skip_field]
    density_curve_id: Option<AssetIdVal<DensityCurve>>,
    #[skip_field]
    weight_sum: f32,
    #[skip_field]
    base_rule_ids: Vec<AssetIdVal<BaseRule>>,
    #[skip_field]
    feature_samples: Vec<FeatureSample>,
}
impl MicroSample {
    /// insert density_curve_id in self if self doesn't contain any density
    pub fn insert_first_density_id(&mut self, density_curve_id: &Option<AssetIdVal<DensityCurve>>) {
        //If self.density_curve_id doesn't exists and parameter density_curve_id exists add
        if self.density_curve_id.is_none() && density_curve_id.is_some() {
            self.density_curve_id = density_curve_id.clone();
        }
    }
    /// normalize values so that weight_sum = 1.0
    pub fn normalize(&mut self) -> Result<(), BaseplateError> {
        if IterableField::<f32>::iter(self).any(|el| !el.is_finite()) {
            return Err(BaseplateError::NonFiniteSample("terrain blend"));
        }

        if self.weight_sum <= f32::EPSILON {
            for el in IterableField::<f32>::iter_mut(self) {
                *el = 0.0;
            }
            self.weight_sum = 0.0;
            return Ok(());
        }
        let weight = self.weight_sum;
        for el in IterableField::<f32>::iter_mut(self) {
            *el /= weight;
        }
        self.weight_sum = 1.0;
        Ok(())
    }
    pub fn set_base_rule_ids(&mut self, base_rule_ids: Vec<AssetIdVal<BaseRule>>) {
        self.base_rule_ids = base_rule_ids;
    }
    pub fn set_feature_samples(&mut self, samples: Vec<FeatureSample>) {
        self.feature_samples = samples;
    }
    pub fn set_field(&mut self, field: EnvironmentField) {
        self.field = field;
    }
    /// Adds a terrain blend to this terrain set.
    ///
    /// A blend is a modification of a terrain that can be applied to a terrain.
    /// They are used to create new terrains with the same base, but different
    /// details.
    ///
    /// The height_mean, height_variance, roughness
    pub fn add_terrain(
        &mut self,
        height_mean: f32,
        height_variance: f32,
        roughness: f32,
        erosion_bias: f32,
        weight: f32,
    ) {
        self.height_mean += height_mean;
        self.height_variance += height_variance;
        self.roughness += roughness;
        self.erosion_bias += erosion_bias;
        self.weight_sum += weight;
    }

    pub fn base_rules(&self) -> &Vec<AssetIdVal<BaseRule>> {
        &self.base_rule_ids
    }
    pub fn feature_samples(&self) -> &[FeatureSample] {
        &self.feature_samples
    }

    pub fn height_mean(&self) -> f32 {
        self.height_mean
    }
    pub fn height_variance(&self) -> f32 {
        self.height_variance
    }
    pub fn roughness(&self) -> f32 {
        self.roughness
    }
    pub fn erosion_bias(&self) -> f32 {
        self.erosion_bias
    }
    pub fn density_curve_id(&self) -> Option<AssetIdVal<DensityCurve>> {
        self.density_curve_id
    }
    pub fn weight_sum(&self) -> f32 {
        self.weight_sum
    }
    pub fn field(&self) -> EnvironmentField {
        self.field
    }
}

//Biome samples

#[derive(Default, Clone)]
pub struct BiomeSample {
    id: AssetIdVal<Biome>,
    weight: f32,
    terrain_profile_id: AssetIdVal<TerrainProfile>,
    base_rule_ids: Vec<AssetIdVal<BaseRule>>,
    sub_sample: Vec<SubBiomeSample>,
}
impl BiomeSample {
    pub(crate) fn new(
        id: AssetIdVal<Biome>,
        weight: f32,
        terrain_profile_id: AssetIdVal<TerrainProfile>,
        base_rule_ids: Vec<AssetIdVal<BaseRule>>,
    ) -> Self {
        Self {
            id,
            weight,
            terrain_profile_id: terrain_profile_id,
            base_rule_ids,
            sub_sample: Vec::new(),
        }
    }
    pub(crate) fn terrain_profile_id(&self) -> AssetIdVal<TerrainProfile> {
        self.terrain_profile_id
    }
    pub(crate) fn weight(&self) -> f32 {
        self.weight
    }

    pub(crate) fn id(&self) -> AssetIdVal<Biome> {
        self.id
    }

    pub(crate) fn push_subsample(&mut self, evaluated_sub: SubBiomeSample) {
        self.sub_sample.push(evaluated_sub);
    }

    pub(crate) fn get_subsample(&self) -> &Vec<SubBiomeSample> {
        &self.sub_sample
    }

    pub fn base_rule_ids(&self) -> &Vec<AssetIdVal<BaseRule>> {
        &self.base_rule_ids
    }
    pub fn mut_rule_ids(&mut self) -> &mut Vec<AssetIdVal<BaseRule>> {
        &mut self.base_rule_ids
    }

    pub fn get_mut_subsample(&mut self) -> &mut Vec<SubBiomeSample> {
        &mut self.sub_sample
    }

    pub fn sort_subbiome_sample(&mut self) {
        self.sub_sample.sort()
    }
}

#[derive(Default, Clone)]
pub struct SubBiomeSample {
    id: AssetIdVal<SubBiome>,
    parent_id: AssetIdVal<Biome>,

    weight: f32,
    terrain_profile_id: Option<AssetIdVal<TerrainProfile>>,
    base_rule_ids: Vec<AssetIdVal<BaseRule>>,
}
impl SubBiomeSample {
    pub(crate) fn id(&self) -> AssetIdVal<SubBiome> {
        self.id
    }
    pub(crate) fn new(
        parent_id: AssetIdVal<Biome>,
        id: AssetIdVal<SubBiome>,
        weight: f32,
        terrain_profile_id: Option<AssetIdVal<TerrainProfile>>,
        base_rule_ids: Vec<AssetIdVal<BaseRule>>,
    ) -> Self {
        Self {
            id,
            parent_id,
            weight,
            terrain_profile_id,
            base_rule_ids,
        }
    }

    pub(crate) fn terrain_profile_id(&self) -> &Option<AssetIdVal<TerrainProfile>> {
        &self.terrain_profile_id
    }

    pub(crate) fn weight(&self) -> f32 {
        self.weight
    }
    pub fn base_rule_ids(&self) -> &Vec<AssetIdVal<BaseRule>> {
        &self.base_rule_ids
    }
    pub fn mut_rule_ids(&mut self) -> &mut Vec<AssetIdVal<BaseRule>> {
        &mut self.base_rule_ids
    }
}
impl PartialEq for SubBiomeSample {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for SubBiomeSample {}

impl PartialOrd for SubBiomeSample {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.id.partial_cmp(&other.id)
    }
}

impl Ord for SubBiomeSample {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id().cmp(&other.id)
    }
}

impl WeightedSample for BiomeSample {
    type Id = AssetIdVal<Biome>;

    fn id(&self) -> Self::Id {
        self.id
    }
    fn weight(&self) -> f32 {
        self.weight
    }
    /// Lerp between two samples.
    fn lerp(&self, other: &Self, t: f32) -> Self {
        let weight = lerp(self.weight, other.weight, t);

        let sub_sample = merge_lerp(&self.sub_sample, &other.sub_sample, t);

        Self {
            id: self.id,
            weight: weight,
            terrain_profile_id: self.terrain_profile_id,
            base_rule_ids: self.base_rule_ids.clone(),
            sub_sample,
        }
    }
    /// Scale a weighted sample.
    fn scaled(&self, w: f32) -> Self {
        let mut sub_samples = self.sub_sample.clone();

        let scale = if self.weight.abs() > f32::EPSILON {
            w / self.weight
        } else {
            0.0
        };

        for sub_sample in &mut sub_samples {
            sub_sample.weight *= scale;
        }

        Self {
            id: self.id,
            weight: w,
            terrain_profile_id: self.terrain_profile_id,
            base_rule_ids: self.base_rule_ids.clone(),
            sub_sample: sub_samples,
        }
    }
}

impl WeightedSample for SubBiomeSample {
    type Id = AssetIdVal<SubBiome>;

    fn id(&self) -> Self::Id {
        self.id
    }
    fn weight(&self) -> f32 {
        self.weight
    }
    ///lerp subbiome sample:
    fn lerp(&self, other: &Self, t: f32) -> Self {
        let weight = lerp(self.weight, other.weight, t);

        Self {
            id: self.id,
            parent_id: self.parent_id, // assume same parent
            weight,
            terrain_profile_id: self.terrain_profile_id, // or resolve conflict
            base_rule_ids: self.base_rule_ids.clone(),
        }
    }
    /// multiply by scale weight
    fn scaled(&self, w: f32) -> Self {
        Self {
            id: self.id,
            parent_id: self.parent_id,
            weight: w,
            terrain_profile_id: self.terrain_profile_id.clone(),
            base_rule_ids: self.base_rule_ids.clone(),
        }
    }
}

pub trait WeightedSample: Sized + Clone {
    type Id: Ord + Copy;

    fn id(&self) -> Self::Id;
    fn weight(&self) -> f32;

    fn lerp(&self, other: &Self, t: f32) -> Self;
    fn scaled(&self, w: f32) -> Self;
}

#[cfg(test)]
mod tests {
    use super::MicroSample;

    #[test]

    fn normalizing_empty_terrain_keeps_finite_zero_values() {
        let mut sample = MicroSample::default();
        sample.normalize().unwrap();

        assert_eq!(sample.height_mean(), 0.0);
        assert_eq!(sample.height_variance(), 0.0);
        assert_eq!(sample.roughness(), 0.0);
        assert_eq!(sample.erosion_bias(), 0.0);
        assert_eq!(sample.weight_sum(), 0.0);
        assert!(sample.density_curve_id().is_none());
    }
}
