//! The player's commands: the only way the world changes from outside (ADR 0034).

use crate::power::PlantKind;
use crate::road::RoadClass;
use crate::zone::Zone;

/// Player actions. Everything that changes the world from outside goes through `World::apply`,
/// so the seed plus a command log is enough to replay a session (`crate::replay`, ADR 0034).
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    SpawnAgents {
        count: u32,
    },
    /// Builds a road of `class`. Roads it follows take that class (ADR 0048).
    BuildRoad {
        ax: f32,
        ay: f32,
        bx: f32,
        by: f32,
        class: RoadClass,
    },
    BuildCurve {
        ax: f32,
        ay: f32,
        cx: f32,
        cy: f32,
        bx: f32,
        by: f32,
        class: RoadClass,
    },
    /// Lays a lattice of `cols` × `rows` blocks of `cell` metres from the origin along the unit
    /// axis `(ux, uy)`; negative counts go the other way, and rows run to the axis's left
    /// (counter-clockwise). Each edge is planned like a hand-drawn road, edges an existing
    /// road runs alongside are left out, and existing roads keep their class. All or nothing
    /// on money; `Alley`, more than `MAX_BLOCKS` either way and cells outside `MIN_CELL` to
    /// `MAX_CELL` are refused (ADR 0049).
    BuildGrid {
        ox: f32,
        oy: f32,
        ux: f32,
        uy: f32,
        cell: f32,
        cols: i32,
        rows: i32,
        class: RoadClass,
    },
    /// Gives the road nearest the point, in bulldoze reach, `class` in place, as drawing over
    /// the whole road would (ADR 0048, 0050). Refused for a road already of `class`.
    SetRoadClass {
        x: f32,
        y: f32,
        class: RoadClass,
    },
    /// Removes the fire station or plant under the point, else the nearest line in reach,
    /// else the building under the point (ADR 0045), else ruins (ADR 0053), else the nearest
    /// road.
    DeleteRoad {
        x: f32,
        y: f32,
    },
    /// Places a power plant at the point; coal snaps beside a road and faces it (ADR 0042).
    /// Refused over roads and other plants, and for coal away from roads.
    PlacePlant {
        kind: PlantKind,
        x: f32,
        y: f32,
    },
    /// Draws a transmission line; each end snaps to a line end, plant or road in reach.
    BuildLine {
        ax: f32,
        ay: f32,
        bx: f32,
        by: f32,
    },
    /// Places a fire station beside the nearest road, facing it (ADR 0053). Refused away from
    /// roads, over roads, plants and stations, and when the city cannot pay.
    PlaceStation {
        x: f32,
        y: f32,
    },
    /// Turns this city's fires on or off (ADR 0053).
    SetFires {
        on: bool,
    },
    /// Sets every zone cell whose centre is inside the rectangle with these corners. `Zone::None`
    /// unzones. See ADR 0014.
    PaintZone {
        ax: f32,
        ay: f32,
        bx: f32,
        by: f32,
        zone: Zone,
    },
    /// Sets every cell of the block containing `(x, y)`; outside every block it changes nothing.
    /// See ADR 0016.
    FillBlock {
        x: f32,
        y: f32,
        zone: Zone,
    },
}
