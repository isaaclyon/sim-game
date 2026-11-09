//! Player actions: the only way the outside changes the world.

use crate::power::PlantKind;
use crate::road::RoadClass;
use crate::water::WaterKind;
use crate::zone::Zone;

/// Player actions. Everything that changes the world from outside goes through `World::apply`,
/// so the seed plus a command log is enough to replay a session (`crate::replay`, ADR 0034).
/// Main roads and dense zones are refused until the town era unlocks them (ADR 0059).
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
    /// Builds a curve of `class`. `chain` marks a continuous curve's later pieces, which join
    /// the previous piece's undo step while that chain is open (ADR 0046, 0051).
    BuildCurve {
        ax: f32,
        ay: f32,
        cx: f32,
        cy: f32,
        bx: f32,
        by: f32,
        class: RoadClass,
        chain: bool,
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
    /// else the pump or tower under the point, else the building under the point (ADR 0045),
    /// else ruins (ADR 0053), else the nearest road.
    DeleteRoad {
        x: f32,
        y: f32,
    },
    /// Places a power plant at the point; coal snaps beside a road and faces it (ADR 0042).
    /// Refused over roads and other plants, for coal away from roads, and when the city
    /// cannot pay (ADR 0052).
    PlacePlant {
        kind: PlantKind,
        x: f32,
        y: f32,
    },
    /// Places a pump or water tower beside the nearest road, facing it (ADR 0052). Refused
    /// away from roads, over roads, plants, pumps and towers, and when the city cannot pay.
    PlaceWater {
        kind: WaterKind,
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
    /// roads, over roads, plants, pumps, towers and stations, and when the city cannot pay.
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
    /// Takes back the newest placeable edit still in the history, refunding what it cost
    /// (ADR 0051). Refused when there is none or it no longer fits the map.
    Undo,
    /// Makes the newest undone edit again, charging what it cost (ADR 0051). Refused when
    /// there is none, it no longer fits the map, or the city cannot pay.
    Redo,
    /// Sets every cell of the block containing `(x, y)`; outside every block it changes nothing.
    /// See ADR 0016.
    FillBlock {
        x: f32,
        y: f32,
        zone: Zone,
    },
}
