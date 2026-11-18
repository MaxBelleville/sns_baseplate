use std::collections::BTreeMap;

use super::assets::{EnvironmentField, NoiseContribution, NoiseField};
use crate::BaseplateError;
use sns_asset_manager::prelude::*;

use sns_helper::{
    IterableField,
    prelude::{Fbm2D, NoiseSampler, NoiseSettings, SimdNoiseBackend, Vec2, check_vec},
};

impl From<&NoiseField> for NoiseSettings {
    fn from(field: &NoiseField) -> Self {
        NoiseSettings {
            frequency: field.frequency(),
            octaves: field.octaves(),
            gain: field.gain(),
            lacunarity: field.lacunarity(),
            mult: field.mult(),
        }
    }
}

impl From<&NoiseField> for NoiseSampler<SimdNoiseBackend> {
    fn from(field: &NoiseField) -> Self {
        NoiseSampler::new(SimdNoiseBackend, NoiseSettings::from(field))
    }
}

pub struct NoiseScaleConfig {
    micro_contribution: NoiseContribution,
    meso_contribution: NoiseContribution,
    macro_contribution: NoiseContribution,
}
impl Default for NoiseScaleConfig {
    fn default() -> Self {
        Self {
            micro_contribution: NoiseContribution::default(),
            meso_contribution: NoiseContribution::default(),
            macro_contribution: NoiseContribution::default(),
        }
    }
}

impl NoiseScaleConfig {
    pub fn new(
        micro_contribution: &NoiseContribution,
        meso_contribution: &NoiseContribution,
        macro_contribution: &NoiseContribution,
    ) -> Self {
        Self {
            micro_contribution: micro_contribution.clone(),
            meso_contribution: meso_contribution.clone(),
            macro_contribution: macro_contribution.clone(),
        }
    }
    pub fn get_macro(&self) -> &NoiseContribution {
        &self.macro_contribution
    }
    pub fn get_meso(&self) -> &NoiseContribution {
        &self.meso_contribution
    }
    pub fn micro_basis(&self) -> &NoiseContribution {
        &self.micro_contribution
    }
    pub fn macro_noise_id(&self) -> &AssetIdVal<NoiseField> {
        self.macro_contribution.noise_id()
    }
    pub fn meso_noise_id(&self) -> &AssetIdVal<NoiseField> {
        self.meso_contribution.noise_id()
    }
    pub fn micro_noise_id(&self) -> &AssetIdVal<NoiseField> {
        self.micro_contribution.noise_id()
    }
    pub fn macro_enviro_id(&self) -> &AssetIdVal<EnvironmentField> {
        self.macro_contribution.environment_id()
    }
    pub fn meso_enviro_id(&self) -> &AssetIdVal<EnvironmentField> {
        self.meso_contribution.environment_id()
    }
    pub fn micro_enviro_id(&self) -> &AssetIdVal<EnvironmentField> {
        self.micro_contribution.environment_id()
    }
}

pub struct FieldGenerator {
    noise_scale: NoiseScaleConfig,
}
impl Default for FieldGenerator {
    fn default() -> Self {
        Self {
            noise_scale: NoiseScaleConfig::default(),
        }
    }
}

impl FieldGenerator {
    pub(crate) fn new(config: NoiseScaleConfig) -> Self {
        Self {
            noise_scale: config,
        }
    }
    pub(crate) fn validate(
        &self,
        enviro_fields: &BTreeMap<AssetIdVal<EnvironmentField>, &EnvironmentField>,
        noise_fields: &BTreeMap<AssetIdVal<NoiseField>, NoiseSampler<SimdNoiseBackend>>,
        issues: &mut Vec<String>,
    ) {
        let vec_noise = vec![
            self.noise_scale.macro_noise_id(),
            self.noise_scale.meso_noise_id(),
            self.noise_scale.micro_noise_id(),
        ];
        let vec_enviro = vec![
            self.noise_scale.macro_enviro_id(),
            self.noise_scale.meso_enviro_id(),
            self.noise_scale.micro_enviro_id(),
        ];

        check_vec::<&AssetIdVal<NoiseField>>(
            issues,
            &vec_noise,
            |id| !noise_fields.contains_key(*id),
            "noise basis references a missing noise field".to_string(),
        );

        check_vec::<&AssetIdVal<EnvironmentField>>(
            issues,
            &vec_enviro,
            |id| !enviro_fields.contains_key(*id),
            "noise basis references a missing environment field".to_string(),
        );

        for (id, field) in enviro_fields {
            if field.validate().is_err() {
                issues.push(format!("environment field {}", id.get()));
            }
        }

        for (id, sampler) in noise_fields {
            validate_noise_settings(
                &format!("noise field {}", id.get()),
                sampler.settings(),
                issues,
            );
        }
    }

    pub fn sample(
        &self,
        coord: Vec2,
        seed: i32,
        enviro_fields: &BTreeMap<AssetIdVal<EnvironmentField>, &EnvironmentField>,
        noise_fields: &BTreeMap<AssetIdVal<NoiseField>, NoiseSampler<SimdNoiseBackend>>,
    ) -> Result<EnvironmentField, BaseplateError> {
        let mut field = EnvironmentField::default();

        let macro_noise = noise_fields
            .get(self.noise_scale.macro_noise_id())
            .ok_or_else(|| BaseplateError::MissingAsset("macro noise field".into()))?;

        let mut noise_val = macro_noise.sample_vec2::<Fbm2D>(coord, seed);

        let macro_weight = enviro_fields
            .get(self.noise_scale.macro_enviro_id())
            .cloned()
            .ok_or_else(|| BaseplateError::MissingAsset("macro environment field".into()))?;

        field = field + (macro_weight.clone() * noise_val);

        let meso_noise = noise_fields
            .get(self.noise_scale.meso_noise_id())
            .ok_or_else(|| BaseplateError::MissingAsset("meso noise field".into()))?;

        noise_val = meso_noise.sample_vec2::<Fbm2D>(coord, seed);

        let meso_weight = enviro_fields
            .get(self.noise_scale.meso_enviro_id())
            .cloned()
            .ok_or_else(|| BaseplateError::MissingAsset("meso environment field".into()))?;

        field = field + (meso_weight.clone() * noise_val);

        let micro_noise = noise_fields
            .get(self.noise_scale.micro_noise_id())
            .ok_or_else(|| BaseplateError::MissingAsset("micro noise field".into()))?;

        noise_val = micro_noise.sample_vec2::<Fbm2D>(coord, seed);

        let micro_weight = enviro_fields
            .get(self.noise_scale.micro_enviro_id())
            .cloned()
            .ok_or_else(|| BaseplateError::MissingAsset("micro environment field".into()))?;

        field = field + (micro_weight.clone() * noise_val);

        field.abs_rugedness();

        if field.validate().is_err() {
            return Err(BaseplateError::NonFiniteSample("environment field"));
        }

        Ok(field)
    }
    pub fn micro_noise_id(&self) -> &AssetIdVal<NoiseField> {
        self.noise_scale.micro_noise_id()
    }
}

fn validate_noise_settings(label: &str, settings: &NoiseSettings, issues: &mut Vec<String>) {
    check_vec::<f32>(
        issues,
        &IterableField::<f32>::iter(settings).copied().collect(),
        |value| !value.is_finite(),
        format!("{label} settings must be finite"),
    );

    if !settings.frequency.is_finite() || settings.frequency <= 0.0 {
        issues.push(format!(
            "{label} frequency must be finite and greater than zero"
        ));
    }
    if settings.octaves == 0 {
        issues.push(format!("{label} octaves must be at least one"));
    }
    if settings.gain.is_finite() && settings.gain < 0.0 {
        issues.push(format!("{label} gain must be nonnegative"));
    } else if settings.octaves > 1 && settings.gain == 0.0 {
        issues.push(format!(
            "{label} gain must be greater than zero with multiple octaves"
        ));
    }
    if settings.lacunarity.is_finite() && settings.lacunarity < 0.0 {
        issues.push(format!("{label} lacunarity must be nonnegative"));
    } else if settings.octaves > 1 && settings.lacunarity == 0.0 {
        issues.push(format!(
            "{label} lacunarity must be greater than zero with multiple octaves"
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::validate_noise_settings;
    use sns_helper::prelude::NoiseSettings;

    fn issues_for(settings: NoiseSettings) -> Vec<String> {
        let mut issues = Vec::new();
        validate_noise_settings("test noise", &settings, &mut issues);
        issues
    }

    #[test]
    fn noise_requires_positive_frequency_and_at_least_one_octave() {
        let issues = issues_for(NoiseSettings::new(0.0, 0, 0.0, 0.0, 1.0));

        assert!(issues.iter().any(|issue| issue.contains("frequency")));
        assert!(issues.iter().any(|issue| issue.contains("octaves")));
    }

    #[test]
    fn single_octave_noise_allows_zero_gain_and_lacunarity() {
        let issues = issues_for(NoiseSettings::new(1.0, 1, 0.0, 0.0, 1.0));

        assert!(issues.is_empty());
    }

    #[test]
    fn multiple_octaves_require_positive_gain_and_lacunarity() {
        let issues = issues_for(NoiseSettings::new(1.0, 2, 0.0, 0.0, 1.0));

        assert!(issues.iter().any(|issue| issue.contains("gain")));
        assert!(issues.iter().any(|issue| issue.contains("lacunarity")));
    }
}
