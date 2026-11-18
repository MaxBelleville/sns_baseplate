use std::collections::BTreeMap;

use crate::BaseplateError;
use crate::samples::{MacroSample, MicroSample};

use crate::assets::{NoiseField, TerrainProfile};
use crate::structs::BiomeSettings;
use sns_asset_manager::prelude::*;

use sns_helper::prelude::{Fbm2D, NoiseSampler, SimdNoiseBackend, Vec2};

#[derive(Default)]
pub struct TerrainPlate<'a> {
    terrain_profiles: BTreeMap<AssetIdVal<TerrainProfile>, &'a TerrainProfile>,
    biome_settings: BiomeSettings,
}
impl<'a> TerrainPlate<'a> {
    pub fn new(
        terrain_profiles: BTreeMap<AssetIdVal<TerrainProfile>, &'a TerrainProfile>,
        biome_settings: BiomeSettings,
    ) -> Self {
        Self {
            terrain_profiles,
            biome_settings,
        }
    }

    pub(crate) fn terrain_profiles(
        &self,
    ) -> &BTreeMap<AssetIdVal<TerrainProfile>, &TerrainProfile> {
        &self.terrain_profiles
    }

    pub(crate) fn validate(
        &self,
        noise_fields: &BTreeMap<AssetIdVal<NoiseField>, NoiseSampler<SimdNoiseBackend>>,
        issues: &mut Vec<String>,
    ) {
        self.biome_settings
            .validate("terrain biome settings", issues);

        for (profile_id, profile) in &self.terrain_profiles {
            if profile.validate().is_err() || !noise_fields.contains_key(profile.base_noise()) {
                issues.push(format!(
                    "terrain prof {} must be finite and have a noise field",
                    profile_id.get()
                ))
            }
        }
    }

    pub fn blend(
        &self,
        coord: Vec2,
        seed: i32,
        macro_sample: MacroSample,
        noise_fields: &BTreeMap<AssetIdVal<NoiseField>, NoiseSampler<SimdNoiseBackend>>,
    ) -> Result<MicroSample, BaseplateError> {
        if !coord.x.is_finite() || !coord.y.is_finite() {
            return Err(BaseplateError::InvalidSamplePosition);
        }

        let mut micro = MicroSample::default();
        micro.set_field(macro_sample.environment());
        micro.set_feature_samples(macro_sample.feature_samples().to_vec());

        micro.set_base_rule_ids(macro_sample.base_rule_ids().clone());

        for biome_samples in macro_sample.biome_samples() {
            self.apply(
                &biome_samples.terrain_profile_id(),
                coord,
                seed,
                biome_samples.weight() * self.biome_settings.biome_base_weight,
                &mut micro,
                noise_fields,
            )?;

            for sub in biome_samples.get_subsample() {
                let Some(terrain_profile_id) = sub.terrain_profile_id() else {
                    continue;
                };

                self.apply(
                    terrain_profile_id,
                    coord,
                    seed,
                    sub.weight() * self.biome_settings.subbiome_base_weight,
                    &mut micro,
                    noise_fields,
                )?;
            }
        }

        micro.normalize()?;

        Ok(micro)
    }

    pub fn apply(
        &self,
        terrain_profile_id: &AssetIdVal<TerrainProfile>,
        coord: Vec2,
        seed: i32,
        weight: f32,
        micro: &mut MicroSample,
        noise_fields: &BTreeMap<AssetIdVal<NoiseField>, NoiseSampler<SimdNoiseBackend>>,
    ) -> Result<(), BaseplateError> {
        if !coord.is_finite() {
            return Err(BaseplateError::InvalidSamplePosition);
        }

        if !weight.is_finite() || weight < 0.0 {
            return Err(BaseplateError::InvalidConfiguration(vec![
                "terrain contribution weight must be finite and nonnegative".into(),
            ]));
        }
        let tp = self
            .terrain_profiles
            .get(terrain_profile_id)
            .ok_or_else(|| {
                BaseplateError::MissingAsset(format!(
                    "terrain profile {}",
                    terrain_profile_id.get()
                ))
            })?;

        let noise = noise_fields
            .get(tp.base_noise())
            .map(|n| n.sample_vec2::<Fbm2D>(coord, seed))
            .ok_or_else(|| {
                BaseplateError::MissingAsset(format!("noise field {}", tp.base_noise().get()))
            })?;

        if !noise.is_finite() {
            return Err(BaseplateError::NonFiniteSample("terrain noise"));
        }

        // Noise perturbs profile values while keeping the multiplier nonnegative.
        let value_scale = 1.0 + noise.clamp(-1.0, 1.0);

        let height_mean = weight * value_scale * tp.height_offset();
        let height_variance = weight * value_scale * tp.height_variance();
        let roughness = weight * value_scale * tp.roughness();
        let erosion_bias = weight * value_scale * tp.erosion_strength();

        if weight > f32::EPSILON {
            micro.insert_first_density_id(tp.density_curve_id());
        }

        micro.add_terrain(
            height_mean,
            height_variance,
            roughness,
            erosion_bias,
            weight,
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::TerrainPlate;
    use crate::samples::MacroSample;
    use sns_helper::prelude::Vec2;
    use std::collections::BTreeMap;

    #[test]
    fn empty_terrain_contributions_produce_a_zero_sample() {
        let plate = TerrainPlate::default();
        let noise_fields = BTreeMap::new();
        let macro_sample = MacroSample::default();
        let sample = plate
            .blend(Vec2::ZERO, 0, macro_sample, &noise_fields)
            .unwrap();

        assert_eq!(sample.height_mean(), 0.0);
        assert_eq!(sample.height_variance(), 0.0);
        assert_eq!(sample.roughness(), 0.0);
        assert_eq!(sample.erosion_bias(), 0.0);
        assert_eq!(sample.weight_sum(), 0.0);
        assert!(sample.density_curve_id().is_none());
    }
}
