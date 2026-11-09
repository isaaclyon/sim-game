//! The grid tool (ADR 0049): one command lays a lattice of blocks, each edge planned like a
//! hand-drawn road, edges along existing roads left out, all or nothing on money.
use sim::rng::Rng;
use sim::road::{RoadClass, RoadNetwork};
use sim::world::{GridPreview, START_FUNDS};
use sim::{Command, Config, Era, World};

#[allow(clippy::too_many_arguments)]
fn grid(
    ox: f32,
    oy: f32,
    ux: f32,
    uy: f32,
    cell: f32,
    cols: i32,
    rows: i32,
    class: RoadClass,
) -> Command {
    Command::BuildGrid {
        ox,
        oy,
        ux,
        uy,
        cell,
        cols,
        rows,
        class,
    }
}

/// Empty land in the town era, so main road grids are unlocked (ADR 0059).
fn empty() -> World {
    World::empty(Config::default(), 30, 30).with_era(Era::Town)
}

/// A small town: a perfect 4 × 4 street lattice 100 m apart, from (0, 0) to (300, 300).
fn town() -> World {
    let net = RoadNetwork::generate_lattice(4, 4, 100.0);
    World::new(Config::default(), net, Rng::new(1))
}

fn ends(w: &World, road: u32) -> ((f32, f32), (f32, f32)) {
    let (a, b) = w.roads.road_ends(road);
    let at = |n: u32| (w.roads.node_x[n as usize], w.roads.node_y[n as usize]);
    (at(a), at(b))
}

/// Pairs of active roads that run alongside each other, within about 11.5° of parallel and
/// closer than 40 m for more than two lot widths (32 m): the slivers. Each lattice edge
/// ignores an overlap under one lot width (16 m), and two edges meet at a lattice point.
/// Measured in 1 m samples of the second road against the first.
fn slivers(w: &World) -> Vec<(u32, u32)> {
    let roads: Vec<u32> = w.roads.active_roads().collect();
    let mut out = Vec::new();
    for (k, &r) in roads.iter().enumerate() {
        let (a, b) = ends(w, r);
        let len = (b.0 - a.0).hypot(b.1 - a.1);
        let d = ((b.0 - a.0) / len, (b.1 - a.1) / len);
        for &s in &roads[k + 1..] {
            let (p, q) = ends(w, s);
            let l = (q.0 - p.0).hypot(q.1 - p.1);
            let e = ((q.0 - p.0) / l, (q.1 - p.1) / l);
            if (d.0 * e.1 - d.1 * e.0).abs() >= 0.2 {
                continue;
            }
            let close = (0..=l as i32)
                .map(|m| (p.0 + e.0 * m as f32, p.1 + e.1 * m as f32))
                .filter(|x| {
                    let along = (x.0 - a.0) * d.0 + (x.1 - a.1) * d.1;
                    let off = ((x.1 - a.1) * d.0 - (x.0 - a.0) * d.1).abs();
                    (0.0..=len).contains(&along) && off < 40.0
                })
                .count();
            if close > 32 {
                out.push((r, s));
            }
        }
    }
    out
}

/// Whether every active road reaches every other through junctions.
fn connected(w: &World) -> bool {
    let mut parent: Vec<usize> = (0..w.roads.node_count()).collect();
    fn find(p: &mut [usize], x: usize) -> usize {
        if p[x] != x {
            let r = find(p, p[x]);
            p[x] = r;
        }
        p[x]
    }
    for r in w.roads.active_roads() {
        let (a, b) = w.roads.road_ends(r);
        let (x, y) = (find(&mut parent, a as usize), find(&mut parent, b as usize));
        parent[x] = y;
    }
    let mut roots: Vec<usize> = w
        .roads
        .active_roads()
        .map(|r| find(&mut parent, w.roads.road_ends(r).0 as usize))
        .collect();
    roots.dedup();
    roots.windows(2).all(|p| p[0] == p[1])
}

fn preview(w: &World, cmd: &Command) -> GridPreview {
    w.grid_preview(cmd).expect("a valid grid")
}

#[test]
fn a_grid_on_empty_land_builds_every_lattice_edge() {
    let mut w = empty();
    let cmd = grid(400.0, 400.0, 1.0, 0.0, 80.0, 3, 2, RoadClass::Street);
    let p = preview(&w, &cmd);
    // (cols + 1)·rows edges across the axis and (rows + 1)·cols along it.
    assert_eq!(p.roads.len(), 4 * 2 + 3 * 3);
    assert!(p.dropped.is_empty());
    assert_eq!(p.cost, Some(17 * 80 * 10), "$10 per metre of street");
    assert!(w.apply(cmd));
    // Every lattice point is a corner or a T or a crossing, so straight joins merge nothing.
    assert_eq!(w.roads.road_count(), 17);
    assert_eq!(w.stats().money.balance, START_FUNDS - 13_600);
    assert!(connected(&w));
    assert!(slivers(&w).is_empty());
}

#[test]
fn negative_counts_lay_the_blocks_on_the_other_sides() {
    let mut w = empty();
    let cmd = grid(400.0, 400.0, 0.0, 1.0, 64.0, -2, -1, RoadClass::Main);
    assert!(w.apply(cmd));
    assert_eq!(w.roads.road_count(), 7);
    let [x0, y0, x1, y1] = w.roads.bounds();
    // Axis +y, so its left (counter-clockwise) side is -x; both counts are negative.
    assert_eq!([x0, y0, x1, y1], [400.0, 272.0, 464.0, 400.0]);
    assert!(
        w.roads
            .active_roads()
            .all(|r| w.roads.road_class(r) == RoadClass::Main)
    );
}

#[test]
fn edges_along_existing_roads_are_left_out_and_keep_their_class() {
    let mut net = RoadNetwork::default();
    let (a, b) = (net.add_node(0.0, 0.0), net.add_node(80.0, 0.0));
    net.add_road(a, b, RoadClass::Street);
    // Parallel to the next edge along, 20 m off: a sliver if both stayed.
    let (c, d) = (net.add_node(80.0, 20.0), net.add_node(160.0, 20.0));
    net.add_road(c, d, RoadClass::Street);
    net.rebuild_adjacency();
    let mut w = World::new(Config::default(), net, Rng::new(1));
    let cmd = grid(0.0, 0.0, 1.0, 0.0, 80.0, 2, 1, RoadClass::Main);
    let p = preview(&w, &cmd);
    assert_eq!(
        p.dropped,
        [[0.0, 0.0, 80.0, 0.0], [80.0, 0.0, 160.0, 0.0]],
        "the edge on the road and the one beside the other road"
    );
    // The cross edge at x = 80 stops at the other road's end on its way: two legs.
    assert_eq!(p.roads.len(), 6);
    assert!(w.apply(cmd));
    assert_eq!(w.roads.road_class(0), RoadClass::Street, "never upgraded");
    assert!(slivers(&w).is_empty(), "{:?}", slivers(&w));
    assert!(connected(&w));
}

#[test]
fn a_grid_dragged_across_a_towns_edge_connects_without_slivers() {
    let mut w = town();
    let before = w.roads.road_count();
    // Starts inside the town and runs east past its edge at x = 300, 30 m off its streets.
    let cmd = grid(250.0, -30.0, 1.0, 0.0, 80.0, 4, 4, RoadClass::Street);
    let p = preview(&w, &cmd);
    assert!(
        !p.dropped.is_empty(),
        "edges beside town streets are left out"
    );
    assert!(
        p.roads.iter().any(|r| r[2] == 2.0 || r[5] == 2.0),
        "joins split town streets"
    );
    let balance = w.stats().money.balance;
    assert!(w.apply(cmd));
    assert!(w.roads.road_count() > before);
    assert_eq!(w.stats().money.balance, balance - p.cost.unwrap());
    assert!(slivers(&w).is_empty(), "slivers: {:?}", slivers(&w));
    assert!(connected(&w));
}

#[test]
fn grids_dragged_over_a_town_at_any_offset_leave_no_slivers() {
    for (ox, oy) in [
        (10.0, 10.0),
        (-35.0, 125.0),
        (170.0, 230.0),
        (285.0, 55.0),
        (330.0, -70.0),
    ] {
        for (cell, (ux, uy), n) in [
            (64.0, (1.0, 0.0), 3),
            (80.0, (0.0, -1.0), -3),
            (100.0, (1.0, 0.0), -2),
            (128.0, (1.0, 0.0), 3),
            // About 5° off the town's streets.
            (96.0, (0.996_194_7, 0.087_155_74), 3),
        ] {
            let mut w = town();
            let cmd = grid(ox, oy, ux, uy, cell, n, 3, RoadClass::Street);
            // A grid wholly beside town streets has nothing left to build and is refused.
            let builds = preview(&w, &cmd).cost.is_some();
            assert_eq!(w.apply(cmd), builds, "{ox} {oy} {cell}");
            let sl: Vec<_> = slivers(&w)
                .iter()
                .map(|&(a, b)| (ends(&w, a), ends(&w, b)))
                .collect();
            assert!(sl.is_empty(), "{ox} {oy} {cell}: {sl:?}");
            let all: Vec<_> = w
                .roads
                .active_roads()
                .skip(24)
                .map(|r| ends(&w, r))
                .collect();
            assert!(connected(&w), "{ox} {oy} {cell} {all:?}");
        }
    }
}

fn jittered_town(seed: u64) -> World {
    let cfg = Config {
        seed,
        ..Config::default()
    };
    World::generate(cfg, 4, 4)
}

#[test]
fn a_grid_over_a_jittered_towns_edge_connects_without_slivers() {
    // The town and grid of `replays/scripted-grid-tool.cmdlog`: 4 × 4 junctions 120 m apart,
    // each moved up to 30 m, some streets missing.
    for seed in 1..=100 {
        let mut w = jittered_town(seed);
        let cmd = grid(280.0, -40.0, 1.0, 0.0, 80.0, 4, 5, RoadClass::Street);
        assert!(w.apply(cmd), "seed {seed}");
        let sl: Vec<_> = slivers(&w)
            .iter()
            .map(|&(a, b)| (ends(&w, a), ends(&w, b)))
            .collect();
        assert!(sl.is_empty(), "seed {seed}: {sl:?}");
        assert!(connected(&w), "seed {seed}");
    }
}

#[test]
fn grids_at_any_angle_over_a_jittered_town_leave_no_slivers() {
    // Every new leg is checked last, so no placement makes a sliver. A few placements in a
    // few hundred leave a piece of grid unconnected; that is not asserted here.
    let grids = [
        grid(-60.0, -60.0, 1.0, 0.0, 96.0, 3, 3, RoadClass::Street),
        grid(100.0, 300.0, 0.0, 1.0, 64.0, 4, -4, RoadClass::Street),
        grid(
            50.0,
            330.0,
            0.98,
            0.198_997_5,
            80.0,
            3,
            -3,
            RoadClass::Street,
        ),
        grid(
            330.0,
            50.0,
            0.939_692_6,
            0.342_020_15,
            112.0,
            2,
            2,
            RoadClass::Street,
        ),
    ];
    for seed in 1..=50 {
        for cmd in &grids {
            let mut w = jittered_town(seed);
            w.apply(cmd.clone());
            let sl: Vec<_> = slivers(&w)
                .iter()
                .map(|&(a, b)| (ends(&w, a), ends(&w, b)))
                .collect();
            assert!(sl.is_empty(), "seed {seed} {cmd:?}: {sl:?}");
        }
    }
}

#[test]
fn the_preview_shows_exactly_what_is_built() {
    let mut w = town();
    let cmd = grid(250.0, -30.0, 1.0, 0.0, 80.0, 3, 2, RoadClass::Main);
    let p = preview(&w, &cmd);
    let hash = w.state_hash();
    assert_eq!(
        w.grid_preview(&cmd),
        Some(p.clone()),
        "previews are repeatable"
    );
    assert_eq!(w.state_hash(), hash, "previews change nothing");
    assert!(w.apply(cmd));
    // Every previewed leg lies on a main road; main roads are all previewed legs.
    let on_main = |x: f32, y: f32| {
        w.roads.active_roads().any(|r| {
            let ((ax, ay), (bx, by)) = ends(&w, r);
            let len = (bx - ax).hypot(by - ay);
            let t = (((x - ax) * (bx - ax) + (y - ay) * (by - ay)) / (len * len)).clamp(0.0, 1.0);
            w.roads.road_class(r) == RoadClass::Main
                && (ax + (bx - ax) * t - x).hypot(ay + (by - ay) * t - y) < 0.05
        })
    };
    for r in &p.roads {
        assert!(on_main((r[0] + r[3]) / 2.0, (r[1] + r[4]) / 2.0), "{r:?}");
    }
    let leg_len: f32 = p
        .roads
        .iter()
        .map(|r| (r[3] - r[0]).hypot(r[4] - r[1]))
        .sum();
    let main_len: f32 = w
        .roads
        .active_roads()
        .filter(|&r| w.roads.road_class(r) == RoadClass::Main)
        .map(|r| w.roads.lane_len[2 * r as usize])
        .sum();
    assert!((leg_len - main_len).abs() < 0.5, "{leg_len} vs {main_len}");
}

#[test]
fn an_unaffordable_grid_is_refused_whole() {
    let mut w = empty();
    let big = grid(0.0, 0.0, 1.0, 0.0, 112.0, 20, 20, RoadClass::Street);
    // 840 edges of 112 m at $10 per metre.
    assert_eq!(preview(&w, &big).cost, Some(940_800));
    let hash = w.state_hash();
    assert!(!w.apply(big));
    assert_eq!(w.state_hash(), hash);
    assert_eq!(
        w.roads.road_count(),
        0,
        "not even the part the city could pay for"
    );
}

#[test]
fn counts_past_twenty_alleys_and_bad_cells_are_refused() {
    let mut w = empty();
    let hash = w.state_hash();
    for cmd in [
        grid(0.0, 0.0, 1.0, 0.0, 80.0, 21, 1, RoadClass::Street),
        grid(0.0, 0.0, 1.0, 0.0, 80.0, 1, -21, RoadClass::Street),
        grid(0.0, 0.0, 1.0, 0.0, 80.0, 0, 1, RoadClass::Street),
        grid(0.0, 0.0, 1.0, 0.0, 80.0, 2, 2, RoadClass::Alley),
        grid(0.0, 0.0, 1.0, 0.0, 39.0, 2, 2, RoadClass::Street),
        grid(0.0, 0.0, 1.0, 0.0, 201.0, 2, 2, RoadClass::Street),
        grid(0.0, 0.0, 2.0, 0.0, 80.0, 2, 2, RoadClass::Street),
        grid(f32::NAN, 0.0, 1.0, 0.0, 80.0, 2, 2, RoadClass::Street),
    ] {
        assert_eq!(w.grid_preview(&cmd), None, "{cmd:?}");
        assert!(!w.apply(cmd));
    }
    assert_eq!(w.state_hash(), hash);
}

#[test]
fn the_command_comes_from_a_guided_edge_and_whole_blocks() {
    let w = town();
    // A first edge from the town's corner, a little off the street's line: the angle snaps.
    let raw = [300.0, 0.0, 7.0, 380.0, 3.0, 7.0];
    let cmd = w.grid_command(
        &raw,
        Some((300.0 + 5.0 * 80.0 + 30.0, -2.4 * 80.0)),
        80.0,
        RoadClass::Main,
    );
    let Some(Command::BuildGrid {
        ox,
        oy,
        ux,
        uy,
        cell,
        cols,
        rows,
        class,
    }) = cmd
    else {
        panic!("{cmd:?}");
    };
    assert_eq!(
        (ox, oy, ux, uy, cell, class),
        (300.0, 0.0, 1.0, 0.0, 80.0, RoadClass::Main)
    );
    assert_eq!((cols, rows), (5, -2), "rounded to whole blocks");
    let far = w.grid_command(&raw, Some((9_000.0, 9_000.0)), 80.0, RoadClass::Street);
    assert!(
        matches!(
            far,
            Some(Command::BuildGrid {
                cols: 20,
                rows: 20,
                ..
            })
        ),
        "{far:?}"
    );
    let none = w.grid_command(&raw, None, 80.0, RoadClass::Street);
    assert!(
        matches!(
            none,
            Some(Command::BuildGrid {
                cols: 1,
                rows: 1,
                ..
            })
        ),
        "{none:?}"
    );
    assert_eq!(
        w.grid_command(&raw[..3], None, 80.0, RoadClass::Street),
        None
    );
    assert_eq!(w.grid_command(&raw, None, 20.0, RoadClass::Street), None);
}

#[test]
fn the_draft_packs_the_edge_and_the_grid() {
    let w = empty();
    let raw = [0.0, 0.0, 0.0, 80.0, 0.0, 0.0];
    let mut out = Vec::new();
    w.write_grid_draft(&raw, Some((170.0, 90.0)), 80.0, RoadClass::Street, &mut out);
    let [n, m, length, _angle, k, cols, rows, cost, r] = out[..9] else {
        panic!("{out:?}");
    };
    assert_eq!(
        [n, m, length, k, cols, rows, cost, r],
        [
            2.0,
            0.0,
            80.0,
            0.0,
            2.0,
            1.0,
            5.0 * 800.0 + 2.0 * 800.0,
            7.0
        ]
    );
    assert_eq!(out.len(), 9 + 4 + 6 * 7);
    let mut alley = Vec::new();
    w.write_grid_draft(
        &raw,
        Some((170.0, 90.0)),
        80.0,
        RoadClass::Alley,
        &mut alley,
    );
    assert!(alley[7].is_nan(), "nothing planned");
}

#[test]
fn grids_are_deterministic() {
    let run = || {
        let mut w = town();
        assert!(w.apply(grid(250.0, -30.0, 0.6, 0.8, 96.0, -3, 5, RoadClass::Street)));
        for _ in 0..200 {
            w.tick();
        }
        w.state_hash()
    };
    assert_eq!(run(), run());
}
