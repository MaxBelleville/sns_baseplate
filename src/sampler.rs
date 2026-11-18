use std::collections::{HashMap, HashSet};

use crate::BaseplateError;
use sns_helper::prelude::{IVec2, Vec2};

pub trait GridSampler<T> {
    fn sample_cell(&mut self, grid_coord: IVec2, seed: i32) -> Result<T, BaseplateError>;
}
#[derive(Default)]
pub struct GridSampleCache<T> {
    cell_res: i32,
    grid_size: f32,
    grid: HashMap<IVec2, T>,
}

impl<T> GridSampleCache<T> {
    pub fn new(cell_res: i32, grid_size: f32) -> Self {
        Self {
            cell_res,
            grid_size,
            grid: HashMap::new(),
        }
    }
    pub fn required_cells(&self, center: IVec2) -> HashSet<IVec2> {
        let mut set = HashSet::new();

        for y in -self.cell_res..=self.cell_res {
            for x in -self.cell_res..=self.cell_res {
                set.insert(center + IVec2::new(x, y));
            }
        }

        set
    }
    fn pos_to_cell(&self, pos: Vec2) -> IVec2 {
        IVec2::new(
            (pos.x / self.grid_size).floor() as i32,
            (pos.y / self.grid_size).floor() as i32,
        )
    }
    fn pos_reduce_to_cell(&self, pos: Vec2) -> Vec2 {
        Vec2::new(pos.x / self.grid_size, pos.y / self.grid_size)
    }
    pub fn grid_size(&self) -> f32 {
        self.grid_size
    }
    pub fn cell_res(&self) -> i32 {
        self.cell_res
    }
    pub fn four_cells(&self, x: i32, y: i32) -> Result<[[&T; 2]; 2], BaseplateError> {
        Ok([
            [
                self.cell(IVec2::new(x, y))?,
                self.cell(IVec2::new(x, y + 1))?,
            ],
            [
                self.cell(IVec2::new(x + 1, y))?,
                self.cell(IVec2::new(x + 1, y + 1))?,
            ],
        ])
    }

    pub fn cell(&self, grid_coord: IVec2) -> Result<&T, BaseplateError> {
        self.grid
            .get(&grid_coord)
            .ok_or(BaseplateError::CacheCellMissing {
                x: grid_coord.x,
                y: grid_coord.y,
            })
    }
    pub fn mut_values(&mut self) -> Vec<&mut T> {
        self.grid.values_mut().collect()
    }

    pub fn interpolate<F>(&self, coord: Vec2, lerp: F) -> Result<T, BaseplateError>
    where
        F: Fn(&T, &T, f32) -> T,
    {
        if !coord.is_finite() {
            return Err(BaseplateError::InvalidSamplePosition);
        }

        if !self.grid_size.is_finite() || self.grid_size <= 0.0 {
            return Err(BaseplateError::InvalidConfiguration(vec![
                "grid cache size must be finite and greater than zero".into(),
            ]));
        }

        let macro_grid_coord = self.pos_reduce_to_cell(coord) - 0.5;
        let val0 = macro_grid_coord.floor().as_ivec2();
        let tval = macro_grid_coord - val0.as_vec2();

        let cells = self.four_cells(val0.x, val0.y)?;

        let interp_cell0 = &lerp(cells[0][0], cells[1][0], tval.x);
        let interp_cell1 = &lerp(cells[0][1], cells[1][1], tval.x);
        Ok(lerp(interp_cell0, interp_cell1, tval.y))
    }

    pub fn update<S: GridSampler<T>>(
        &mut self,
        sampler: &mut S,
        world_pos: Vec2,
        seed: i32,
    ) -> Result<(), BaseplateError> {
        if !world_pos.is_finite() {
            return Err(BaseplateError::InvalidSamplePosition);
        }

        if !self.grid_size.is_finite() || self.grid_size <= 0.0 || self.cell_res < 0 {
            return Err(BaseplateError::InvalidConfiguration(vec![
                "grid cache requires a positive finite grid size and nonnegative cell resolution"
                    .into(),
            ]));
        }

        let center = self.pos_to_cell(world_pos);
        let required = self.required_cells(center);

        let mut missing = Vec::new();
        for &grid_coord in &required {
            if !self.grid.contains_key(&grid_coord) {
                let value = sampler.sample_cell(grid_coord, seed)?;
                missing.push((grid_coord, value));
            }
        }

        self.grid.retain(|coord, _| required.contains(coord));
        self.grid.extend(missing);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{GridSampleCache, GridSampler};
    use crate::BaseplateError;
    use sns_helper::prelude::{IVec2, Vec2};

    struct TestSampler;
    impl GridSampler<i32> for TestSampler {
        fn sample_cell(&mut self, _grid_coord: IVec2, _seed: i32) -> Result<i32, BaseplateError> {
            Ok(0)
        }
    }

    struct FailingSampler;
    impl GridSampler<i32> for FailingSampler {
        fn sample_cell(&mut self, grid_coord: IVec2, _seed: i32) -> Result<i32, BaseplateError> {
            if grid_coord.x == 10 {
                Err(BaseplateError::MissingAsset("test cell".into()))
            } else {
                Ok(1)
            }
        }
    }

    #[test]
    fn interpolation_reports_missing_cache_cells() {
        let cache = GridSampleCache::<i32>::new(1, 1.0);
        assert!(matches!(
            cache.interpolate(Vec2::ZERO, |a, _, _| *a),
            Err(BaseplateError::CacheCellMissing { .. })
        ));
    }

    #[test]
    fn update_rejects_non_finite_positions() {
        let mut cache = GridSampleCache::<i32>::new(1, 1.0);
        assert_eq!(
            cache.update(&mut TestSampler, Vec2::splat(f32::NAN), 0),
            Err(BaseplateError::InvalidSamplePosition)
        );
    }

    #[test]
    fn failed_update_preserves_previous_cache_contents() {
        let mut cache = GridSampleCache::<i32>::new(0, 1.0);
        let mut sampler = FailingSampler;

        cache.update(&mut sampler, Vec2::ZERO, 0).unwrap();
        assert_eq!(*cache.cell(IVec2::ZERO).unwrap(), 1);

        assert!(cache.update(&mut sampler, Vec2::new(10.0, 0.0), 0).is_err());
        assert_eq!(*cache.cell(IVec2::ZERO).unwrap(), 1);
        assert!(cache.cell(IVec2::new(10, 0)).is_err());
    }
}
