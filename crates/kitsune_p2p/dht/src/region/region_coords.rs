use crate::coords::{SpaceCoord, SpaceSegment, SpacetimeCoords, TimeCoord, TimeSegment};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, derive_more::Constructor)]
pub struct RegionCoords {
    pub space: SpaceSegment,
    pub time: TimeSegment,
}

impl RegionCoords {
    /// TODO: does this need to map to the actual absolute values, i.e. undergo
    /// topological transformation, or is this correct?
    pub fn to_bounds(&self) -> RegionBounds {
        RegionBounds {
            x: self.space.bounds(),
            t: self.time.bounds(),
        }
    }

    pub fn contains(&self, coords: &SpacetimeCoords) -> bool {
        self.space.contains(coords.space) && self.time.contains(coords.time)
    }
}

#[derive(Debug)]
pub struct RegionBounds {
    pub x: (SpaceCoord, SpaceCoord),
    pub t: (TimeCoord, TimeCoord),
}