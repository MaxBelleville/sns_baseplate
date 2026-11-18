# `sns_baseplate` model guide

## Purpose

`sns_baseplate` provides data models and sampling logic for procedural 2D world geography. Given a world position and seed, the intended pipeline combines layered environmental noise, a large-scale zone archetype, weighted biomes and sub-biomes, and terrain profiles. Samples carry asset IDs and weights so callers can use the result to construct or render a world.

This crate is a library component, not a complete game or terrain renderer. It uses `sns_asset_manager` asset handles and `sns_helper` math/noise types. Configuration structs are asset-oriented; most pipeline implementation modules are private. The crate's `prelude` re-exports the types from `structs`.

## Scale model

The code describes three procedural scales:

1. **Macro (broad climate and geography):** `FieldGenerator` combines macro, meso, and micro noise samples with corresponding `EnvironmentField` weights to produce temperature, moisture, continentalness, and ruggedness. Ruggedness is made absolute after combining the layers. `ZonePlate` finds the nearest seeded, jittered Voronoi site and selects a `ZoneArchetype`; its bias is added to the environmental field. That biased field is used to sample biomes.
2. **Meso (biome composition):** `MesoPlate` evaluates each biome against the climate's temperature/moisture center and width using a Gaussian weight, then normalizes the biome weights. Eligible sub-biomes are selected by comparing a seeded, offset noise mask with their threshold. Their weights depend on the parent biome's normalized weight, configured sub-biome weight, and mask value.
3. **Micro (local terrain):** `MicroGridSampler` jitters each micro-grid point deterministically from its grid coordinate and seed, interpolates a `MacroSample` from the macro cache, then asks `TerrainPlate` to blend the contributing terrain profiles. Profile values contribute height, variance, roughness, and erosion bias; the micro sample also retains an optional density curve ID and the environmental field.

`MacroSample`, `MesoSample`, and `MicroSample` are sample/result types, not the pipeline stages themselves. In current code, meso zone sampling is used inside `MacroPipeline`; `MesoSample` stores the chosen zone site, archetype ID, and edge factor.

## Main data types

- **`EnvironmentField`:** Four scalar environmental dimensions: temperature, moisture, continentalness, ruggedness. It supports field math and scalar multiplication.
- **`PartialEnviroField`:** Temperature and moisture only; used for biome matching.
- **`NoiseField`:** FBM noise parameters (frequency, octaves, gain, lacunarity, multiplier). `noise.rs` converts it to the helper crate's `NoiseSampler` settings.
- **`NoiseBasis` / `NoiseScaleConfig`:** Link noise assets to environmental-field assets for the macro, meso, and micro climate layers.
- **`ZoneArchetype`:** Environmental bias, optional maximum-delta reference, and base-rule references for a broad zone.
- **`Biome` / `SubBiome`:** Climate center/width, parent relationship, selection threshold/weight, terrain profile references, and base-rule references.
- **`TerrainProfile`:** Base noise plus height offset, height variance, roughness, erosion strength, and an optional density curve.
- **`DensityCurve` / `DensityPoint`:** Height-to-density control points. This crate stores them; it does not evaluate the curve in the shown sampling pipeline.
- **`BaseRule`:** Minimum source-weight threshold, optional contribution multiplier (default 1), feature-rule references, and nested all/any/not conditions.
- **`FeatureDefinition` / `FeatureRule`:** Asset-backed feature identity with optional default chance (0.5 when omitted); a feature rule can override that chance, softly avoid sub-biomes, and define veto conditions.
- **`FeatureSample` / `FeatureContribution`:** Final feature chance and source-level attribution for zone, biome, and sub-biome support. These feed downstream placement; baseplate does not place features.
- **`MacroSample`:** Environmental result, weighted biome samples, zone archetype ID, zone rule IDs, and aggregated feature samples.
- **`BiomeSample` / `SubBiomeSample`:** Weighted biome and sub-biome contributions with terrain profile and base-rule references.
- **`MicroSample`:** Blended terrain properties and environment for a local sample.
- **`GridSampleCache<T>`:** Sliding cache of grid-cell samples. `update` retains a square neighborhood around the requested position and samples missing cells through a `GridSampler` implementation. `interpolate` bilinearly combines the four surrounding cached cells.

Asset-backed models derive `MakeAsset` and hold typed `AssetIdVal<T>` references. `WorldPlate::try_new` accepts resolved maps of definitions and noise samplers, validates their references and settings, and returns a `BaseplateError` containing detected configuration issues. `load_baseplates` resolves definitions from the registry and returns the assembled `WorldPlate`; its definition references borrow the registry.

## Sampling flow

```text
world position + seed
        |
        v
layered environment noise ----> zone Voronoi site / archetype bias
                                         |
                                         v
                              climate-weighted biomes
                              + thresholded sub-biomes
                                         |
                                         v
                               cached MacroSample grid
                                         |
                           interpolate at jittered micro point
                                         |
                                         v
                        weighted TerrainProfile contributions
                                         |
                                         v
                                    MicroSample
```

`WorldPlate::macro_update` and `WorldPlate::micro_update` return `Result`. Update the macro cache before the micro cache: interpolation reports a `CacheCellMissing` error if any of its four neighboring macro cells are unavailable. Cache updates also reject non-finite positions.

`WorldPlate::macro_sample_at` interpolates cached macro outputs and returns feature samples without running terrain-profile blending. Feature chances and source support attributions blend across macro-cell boundaries.

## Weight and interpolation behavior

- `merge_lerp` merges two **ID-sorted** weighted sample slices. Matching IDs interpolate their weights; IDs present on only one side are scaled by `1 - t` or `t`. Near-zero results are omitted. Inputs should be sorted by ID for this merge algorithm to work correctly.
- Biome weights are normalized by the sum of positive Gaussian weights. Sub-biome weights are derived from the normalized parent weight and are not subsequently normalized across sub-biomes.
- Terrain properties accumulate weighted contributions and `MicroSample::normalize` divides by the sum of profile weights when positive. If no terrain contributes, it returns finite zero terrain values, zero accumulated weight, and no density curve. The density curve ID is the first available one encountered; competing curves are not blended.
- Base rules use zone edge factor or biome/sub-biome sample weight, multiplied by rule weight and fixed scope priorities of 4:2:1 (zone:biome:sub-biome). Final feature chance is the support-weighted mean of active contributions.
- A feature-rule chance overrides the feature default. Soft avoids reduce support by the combined avoided sub-biome weight. A zone veto suppresses narrower contributions; a biome veto suppresses sub-biome contributions under that biome.
- Seed-based hashing is used for zone-site jitter, zone archetype choice, sub-biome offsets, and micro-grid jitter to make spatial decisions reproducible for a given seed and configuration.

## Error and fallback behavior

- Construct a `WorldPlate` with `WorldPlate::try_new`. It validates required noise/environment fields, zone archetypes and bias references, biome/sub-biome/profile references, finite numeric settings, positive noise frequency, at least one noise octave, positive gain and lacunarity for multi-octave noise, positive biome widths, positive finite zone/cache sizes, and nonnegative cache resolutions and blend weights. It reports all detected configuration issues together as `BaseplateError::InvalidConfiguration`.
- Runtime sampling and cache updates return `Result` instead of panicking for missing required assets, missing cache cells, non-finite sample positions, or non-finite generated values. Callers should handle these errors at the world-generation boundary.
- At least one biome must be configured, and sampling returns a configuration error when no biome receives positive weight. If no positive terrain contribution is available, terrain blending produces zero-valued finite terrain output rather than NaNs. Zero blend weights are valid configuration.
- Optional sub-biome terrain profile references may be absent. Required biome terrain profiles and terrain-profile noise references must resolve during validation.
- Rule thresholds, rule weights, feature chances, and numeric condition values are validated during `WorldPlate` construction. Add feature rules and definitions with `BaseRules::with_feature_assets`; missing references are reported as configuration errors.

## Where to look

- `src/structs.rs`: asset/configuration models, rule evaluation, and rule validation.
- `src/error.rs`: public error types for configuration, loading, and sampling.
- `src/noise.rs`: layered climate field generation, validation, and noise conversion.
- `src/bp/zone_bp.rs`: seeded Voronoi zone selection and zone validation.
- `src/bp/meso_bp.rs`: biome/sub-biome sampling and definition validation.
- `src/bp/macro_bp.rs`: macro pipeline, validation, and macro sample cache.
- `src/bp/terrain_bp.rs`: terrain profile validation and blending.
- `src/bp/micro_bp.rs`: local sampling, jitter, and interpolation from the macro cache.
- `src/bp/mod.rs`: public fallible `WorldPlate` builder and update coordination.
- `src/sampler.rs`: reusable grid cache and bilinear interpolation.
- `src/samples.rs`: sample result types and weighted-sample interpolation.
