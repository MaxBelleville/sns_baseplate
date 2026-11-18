use std::collections::BTreeMap;

use crate::BaseplateError;
use crate::samples::{BiomeSample, SubBiomeSample};

use crate::assets::{Biome, SubBiome, TerrainProfile};
use crate::structs::{BiomeSettings, PartialEnviroField};
use sns_asset_manager::prelude::*;

use sns_helper::prelude::{Fbm2D, NoiseSampler, SimdNoiseBackend, Vec2, hash_id_vec2};
#[derive(Default)]
pub struct MesoPlate<'a> {
    biomes: BTreeMap<AssetIdVal<Biome>, &'a Biome>,
    sub_biomes: BTreeMap<AssetIdVal<SubBiome>, &'a SubBiome>,
    settings: BiomeSettings,
}

impl<'a> MesoPlate<'a> {
    pub fn new(
        biomes: BTreeMap<AssetIdVal<Biome>, &'a Biome>,
        sub_biomes: BTreeMap<AssetIdVal<SubBiome>, &'a SubBiome>,
        settings: BiomeSettings,
    ) -> Self {
        Self {
            biomes,
            sub_biomes,
            settings,
        }
    }

    pub(crate) fn validate(
        &self,
        terrain_profiles: &BTreeMap<AssetIdVal<TerrainProfile>, &'a TerrainProfile>,
        issues: &mut Vec<String>,
    ) {
        self.settings.validate("biome settings", issues);
        if self.biomes.is_empty() {
            issues.push("at least one biome must be configured".into());
        }
        for (biome_id, biome) in &self.biomes {
            biome
                .center()
                .validate(&format!("biome {} center", biome_id.get()), issues);
            biome
                .width()
                .validate(&format!("biome {} width", biome_id.get()), issues);
            let width = biome.width();
            if width.get_temperature() <= 0.0 || width.get_moisture() <= 0.0 {
                issues.push(format!(
                    "biome {} widths must be greater than zero",
                    biome_id.get()
                ));
            }
            if !terrain_profiles.contains_key(biome.terrain_profile_id()) {
                issues.push(format!(
                    "biome {} references a missing terrain profile",
                    biome_id.get()
                ));
            }
            let mut seen_sub_ids = std::collections::BTreeSet::new();
            for sub_id in biome.sub_ids() {
                if !seen_sub_ids.insert(sub_id) {
                    issues.push(format!(
                        "biome {} lists sub-biome {} more than once",
                        biome_id.get(),
                        sub_id.get()
                    ));
                }
                match self.sub_biomes.get(sub_id) {
                    Some(sub) if sub.parent_id() != biome_id => issues.push(format!(
                        "sub-biome {} parent does not match biome {}",
                        sub_id.get(),
                        biome_id.get()
                    )),
                    None => issues.push(format!(
                        "biome {} references missing sub-biome {}",
                        biome_id.get(),
                        sub_id.get()
                    )),
                    _ => {}
                }
            }
        }
        for (sub_id, sub) in &self.sub_biomes {
            if !sub.threshold().is_finite() || sub.threshold() < 0.0 {
                issues.push(format!(
                    "sub-biome {} threshold must be finite and nonnegative",
                    sub_id.get()
                ));
            }
            if !sub.weight().is_finite() || sub.weight() < 0.0 {
                issues.push(format!(
                    "sub-biome {} weight must be finite and nonnegative",
                    sub_id.get()
                ));
            }
            if !self.biomes.contains_key(sub.parent_id()) {
                issues.push(format!(
                    "sub-biome {} references a missing parent biome",
                    sub_id.get()
                ));
            }
            if let Some(profile_id) = sub.terrain_profile_id()
                && !terrain_profiles.contains_key(profile_id)
            {
                issues.push(format!(
                    "sub-biome {} references a missing terrain profile",
                    sub_id.get()
                ));
            }
        }
    }

    pub fn sample(
        &self,
        enviro: PartialEnviroField,
        coord: Vec2,
        seed: i32,
        micro_noise: &NoiseSampler<SimdNoiseBackend>,
    ) -> Result<Vec<BiomeSample>, BaseplateError> {
        if !coord.x.is_finite() || !coord.y.is_finite() {
            return Err(BaseplateError::InvalidSamplePosition);
        }
        if !enviro.get_temperature().is_finite() || !enviro.get_moisture().is_finite() {
            return Err(BaseplateError::NonFiniteSample("biome input environment"));
        }
        let mut out = Vec::new();
        let mut sum = 0.0;

        for (id, biome) in &self.biomes {
            let width = biome.width();

            if width.get_temperature() <= 0.0 || width.get_moisture() <= 0.0 {
                return Err(BaseplateError::InvalidConfiguration(vec![format!(
                    "biome {} widths must be greater than zero",
                    id.get()
                )]));
            }

            let weight = enviro.gaussian(biome.center(), biome.width());

            if !weight.is_finite() {
                return Err(BaseplateError::NonFiniteSample("biome weight"));
            }

            if weight <= f32::EPSILON {
                continue;
            }

            sum += weight;
            if !sum.is_finite() {
                return Err(BaseplateError::NonFiniteSample("biome weight sum"));
            }

            let sample = BiomeSample::new(
                id.clone(),
                weight,
                biome.terrain_profile_id().clone(),
                biome.base_rule_ids().clone(),
            );
            out.push(sample);
        }
        //Normalize values and get sub biomes
        if sum <= f32::EPSILON {
            return Err(BaseplateError::InvalidConfiguration(vec![
                "no biome has a positive sample weight at this position".into(),
            ]));
        }
        for sample in &mut out {
            let mut weight = sample.weight();
            weight /= sum;

            let biome = self.biomes.get(&sample.id()).ok_or_else(|| {
                BaseplateError::MissingAsset(format!("biome {}", sample.id().get()))
            })?;

            for sub_ids in biome.sub_ids() {
                if let Some(evaluated_sub) =
                    self.subbiome_sample(sub_ids, coord, seed, weight, micro_noise)?
                {
                    sample.push_subsample(evaluated_sub);
                }
            }
            sample.sort_subbiome_sample();
        }

        Ok(out)
    }

    pub fn subbiome_sample(
        &self,
        sub_id: &AssetIdVal<SubBiome>,
        coord: Vec2,
        seed: i32,
        parent_weight: f32,
        micro_noise: &NoiseSampler<SimdNoiseBackend>,
    ) -> Result<Option<SubBiomeSample>, BaseplateError> {
        if !coord.x.is_finite() || !coord.y.is_finite() {
            return Err(BaseplateError::InvalidSamplePosition);
        }
        let sub = self
            .sub_biomes
            .get(sub_id)
            .ok_or_else(|| BaseplateError::MissingAsset(format!("sub-biome {}", sub_id.get())))?;

        if sub.validate().is_err() {
            return Err(BaseplateError::InvalidConfiguration(vec![format!(
                "sub-biome {} threshold and weight must be finite and nonnegative",
                sub_id.get()
            )]));
        }
        let hash = hash_id_vec2(sub_id.get(), seed);

        // Centered, small, frequency-aware offset
        let offset = (hash - Vec2::splat(0.5)) * self.settings.subbiome_offset_scale;

        let mask = micro_noise.sample_vec2::<Fbm2D>(coord + offset, seed);

        if !mask.is_finite() {
            return Err(BaseplateError::NonFiniteSample("sub-biome mask"));
        }

        if mask < sub.threshold() {
            return Ok(None);
        }

        let weight = parent_weight * sub.weight() * mask;
        if !weight.is_finite() {
            return Err(BaseplateError::NonFiniteSample("sub-biome weight"));
        }

        Ok(Some(SubBiomeSample::new(
            sub.parent_id().clone(),
            sub_id.clone(),
            weight,
            sub.terrain_profile_id().clone(),
            sub.base_rule_ids().clone(),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::MesoPlate;
    use crate::structs::PartialEnviroField;
    use sns_helper::prelude::NoiseSampler;
    use sns_helper::prelude::{NoiseSettings, SimdNoiseBackend, Vec2};

    #[test]
    fn no_matching_biomes_returns_a_configuration_error() {
        let plate = MesoPlate::default();
        let noise = NoiseSampler::new(SimdNoiseBackend, NoiseSettings::new(1.0, 1, 0.5, 2.0, 1.0));

        let result = plate.sample(PartialEnviroField::default(), Vec2::ZERO, 0, &noise);

        assert!(matches!(
            result,
            Err(crate::BaseplateError::InvalidConfiguration(_))
        ));
    }
}
