use crate::BaseplateError;
use crate::samples::MesoSample;
use crate::structs::{VoronoiResult, ZoneSettings};
use sns_asset_manager::prelude::*;
use std::collections::BTreeMap;

use crate::assets::{EnvironmentField, ZoneArchetype};
use sns_helper::prelude::{IVec2, Vec2, check_btree, check_vec, hash_vec2, hash2};

#[derive(Default)]
pub struct ZonePlate<'a> {
    //Node id: node
    archetypes: BTreeMap<AssetIdVal<ZoneArchetype>, &'a ZoneArchetype>,
    archetypes_ids: Vec<&'a GlossaryItem<ZoneArchetype>>, //Collect and sort keys when load in archetypes from file.
    zone_settings: ZoneSettings,
}

impl<'a> ZonePlate<'a> {
    pub(crate) fn validate(
        &self,
        enviro_fields: &BTreeMap<AssetIdVal<EnvironmentField>, &EnvironmentField>,
        issues: &mut Vec<String>,
    ) {
        if !self.zone_settings.zone_size.is_finite() || self.zone_settings.zone_size <= 0.0 {
            issues.push("zone size must be finite and greater than zero".into());
        }

        if self.archetypes_ids.is_empty() {
            issues.push("at least one zone archetype must be selectable".into());
        }

        check_vec::<&GlossaryItem<ZoneArchetype>>(
            issues,
            &self.archetypes_ids,
            |id| !self.archetypes.contains_key(&id.get_val()),
            format!("selectable zone archetype has no definition"),
        );

        check_btree::<_, _>(
            issues,
            &self.archetypes,
            |_id, archetype| {
                !enviro_fields.contains_key(archetype.bias_id())
                    || archetype
                        .max_delta_id()
                        .as_ref()
                        .is_some_and(|id| !enviro_fields.contains_key(id))
            },
            "zone archetype references a missing bias field".to_string(),
        );
    }

    pub fn new(
        archetypes: BTreeMap<AssetIdVal<ZoneArchetype>, &'a ZoneArchetype>,
        archetypes_ids: Vec<&'a GlossaryItem<ZoneArchetype>>,
        zone_settings: ZoneSettings,
    ) -> Self {
        Self {
            archetypes,
            archetypes_ids,
            zone_settings,
        }
    }

    pub fn get_archetype(&self, id: AssetIdVal<ZoneArchetype>) -> Option<&ZoneArchetype> {
        self.archetypes.get(&id).cloned()
    }

    pub fn sample(&self, coord: Vec2, seed: i32) -> Result<MesoSample, BaseplateError> {
        if !coord.x.is_finite() || !coord.y.is_finite() {
            return Err(BaseplateError::InvalidSamplePosition);
        }

        if !self.zone_settings.zone_size.is_finite() || self.zone_settings.zone_size <= 0.0 {
            return Err(BaseplateError::InvalidConfiguration(vec![
                "zone size must be finite and greater than zero".into(),
            ]));
        }

        let v = self.voronoi_query(coord, seed);
        let archetype_id = self.get_archetype_id(v.closest, seed)?;

        if !self.archetypes.contains_key(&archetype_id) {
            return Err(BaseplateError::MissingAsset(format!(
                "zone archetype {}",
                archetype_id.get()
            )));
        }

        let edge_factor =
            ((v.d2.sqrt() - v.d1.sqrt()) / self.zone_settings.zone_size).clamp(0.0, 1.0);

        if !edge_factor.is_finite() {
            return Err(BaseplateError::NonFiniteSample("zone edge factor"));
        }

        Ok(MesoSample::new(v.closest, archetype_id, edge_factor))
    }

    fn voronoi_query(&self, pos: Vec2, seed: i32) -> VoronoiResult {
        let base = self.pos_to_region_cell(pos);

        let mut closest = base;
        let mut d1 = f32::INFINITY;
        let mut d2 = f32::INFINITY;

        for dy in -1..=1 {
            for dx in -1..=1 {
                let c = base + IVec2::new(dx, dy);
                let jitter_center = hash_vec2(c, seed) * self.zone_settings.zone_size;
                let d = jitter_center.distance_squared(pos);

                if d < d1 {
                    d2 = d1;
                    d1 = d;
                    closest = c;
                } else if d < d2 {
                    d2 = d;
                }
            }
        }

        VoronoiResult { closest, d1, d2 }
    }

    fn get_archetype_id(
        &self,
        coord: IVec2,
        seed: i32,
    ) -> Result<AssetIdVal<ZoneArchetype>, BaseplateError> {
        if self.archetypes_ids.is_empty() {
            return Err(BaseplateError::InvalidConfiguration(vec![
                "at least one zone archetype must be selectable".into(),
            ]));
        }

        let h = hash2(coord, seed ^ 0xA53C9E);
        let idx = (h as usize) % self.archetypes_ids.len();
        Ok(self.archetypes_ids[idx].get_val())
    }

    fn pos_to_region_cell(&self, pos: Vec2) -> IVec2 {
        IVec2::new(
            (pos.x / self.zone_settings.zone_size).floor() as i32,
            (pos.y / self.zone_settings.zone_size).floor() as i32,
        )
    }
}
