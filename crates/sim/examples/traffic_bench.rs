//! Headless traffic benchmark.
//!
//!     cargo run --release -p sim --example traffic_bench -- --agents 50000 --ticks 36000
//!     cargo run --release -p sim --example traffic_bench -- --layout organic --max-despawns 9 --seed 2
//!     cargo run --release -p sim --example traffic_bench -- --scale la
//!     cargo run --release -p sim --example traffic_bench -- --layout perfect --zone
//!
//! `--layout grid` (default) is the generated street grid with avenues. `--layout organic` is a
//! radial town drawn street by street through `Command::BuildRoad`, the way a player builds:
//! rings joined by spokes, with gaps, T-junctions and 5-way junctions, and no avenues.
//! `--layout perfect` is a `--grid` × `--grid` lattice of identical square blocks with no
//! jitter, no gaps and no avenues: the cleanest case for traffic and lots.
//! `--zone` paints the whole map residential before timing and reports how long that took.
//! `--max-despawns N` exits with an error if more than N cars hit the stuck limit.
//! `--warmup N` runs N ticks before timing starts, so start-up path bursts don't skew the mean.
//!
//! `--scale la` is the scale target from ADR 0012: the City of Los Angeles at rush hour, about
//! 250k cars driving at once on about 10,000 km of street. It sets `--grid 200` (8,600 km),
//! `--agents 250000`, `--warmup 6000` and `--ticks 6000`; explicit flags still override.
//!
//! The browser runs 10 ticks per sim second, so a tick under 10 ms leaves room for 10x speed.

// Benchmarks measure wall time and report to stdout; neither feeds sim state.
#![allow(clippy::disallowed_methods, clippy::print_stdout)]

use std::f32::consts::TAU;
use std::time::Instant;

use sim::rng::Rng;
use sim::road::{RoadClass, RoadNetwork};
use sim::{Command, Config, World, Zone};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let value = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
    };
    let arg = |name: &str, default: u32| -> u32 {
        value(name).and_then(|v| v.parse().ok()).unwrap_or(default)
    };
    let la = match value("--scale").map(String::as_str) {
        None => false,
        Some("la") => true,
        Some(other) => panic!("unknown --scale {other}"),
    };
    let agents = arg("--agents", if la { 250_000 } else { 50_000 });
    let grid = arg("--grid", if la { 200 } else { 60 });
    let ticks = arg("--ticks", 6_000);
    let warmup = arg("--warmup", if la { 6_000 } else { 0 });
    let max_despawns = value("--max-despawns").and_then(|v| v.parse::<u64>().ok());
    let layout = value("--layout").map_or("grid", String::as_str);

    let cfg = Config {
        seed: arg("--seed", 1) as u64,
        ..Config::default()
    };
    let dt = cfg.dt;
    let mut world = match layout {
        "grid" => World::generate(cfg, grid, grid),
        "organic" => organic_town(cfg),
        "perfect" => {
            let net = RoadNetwork::generate_lattice(grid, grid, cfg.grid_spacing);
            let seed = cfg.seed;
            World::new(cfg, net, Rng::new(seed))
        }
        other => panic!("unknown --layout {other}"),
    };
    if args.iter().any(|a| a == "--zone") {
        let [ax, ay, bx, by] = world.roads.bounds();
        let t = Instant::now();
        world.apply(Command::PaintZone {
            ax,
            ay,
            bx,
            by,
            zone: Zone::ResidentialLow,
        });
        println!(
            "zoning    whole map in {:.0} ms | {} lots",
            t.elapsed().as_secs_f64() * 1000.0,
            world.lots.active_lots().count()
        );
    }
    world.apply(Command::SpawnAgents { count: agents });
    println!(
        "{layout}: {} nodes, {} roads | {agents} agents | {warmup} warmup + {ticks} timed ticks ({:.0} sim min)",
        world.roads.node_count(),
        world.roads.road_count(),
        (warmup + ticks) as f32 * dt / 60.0
    );

    for _ in 0..warmup {
        world.tick();
    }
    let mut times = Vec::with_capacity(ticks as usize);
    let (mut peak_driving, mut driving_sum, mut driving_samples) = (0, 0u64, 0u64);
    let start = Instant::now();
    for i in 0..ticks {
        let t = Instant::now();
        world.tick();
        times.push(t.elapsed().as_secs_f64() * 1000.0);
        if i % 100 == 0 {
            let driving = world.stats().driving;
            peak_driving = peak_driving.max(driving);
            driving_sum += u64::from(driving);
            driving_samples += 1;
        }
    }
    let wall = start.elapsed().as_secs_f64();

    let mean = times.iter().sum::<f64>() / times.len() as f64;
    times.sort_by(f64::total_cmp);
    let pct = |p: f64| times[((times.len() - 1) as f64 * p) as usize];
    let s = world.stats();
    println!(
        "tick ms   mean {mean:.2} | p50 {:.2} | p99 {:.2} | max {:.2}",
        pct(0.5),
        pct(0.99),
        pct(1.0)
    );
    println!(
        "speed     {:.0}x realtime (sim {:.0}s in {wall:.1}s wall) | 10x needs 10 ms/tick in WASM, about 4 ms native",
        ticks as f64 * dt as f64 / wall,
        ticks as f64 * dt as f64
    );
    println!(
        "traffic   mean driving {} | peak driving {peak_driving}",
        driving_sum / driving_samples.max(1)
    );
    println!(
        "          trips {} | paths ok {} failed {} | stuck despawns {} | detours {}",
        s.trips_completed, s.paths_found, s.paths_failed, s.despawned_stuck, s.detours
    );
    println!(
        "routing   {:.0} nodes expanded per route search",
        s.path_nodes_expanded as f64 / (s.paths_found + s.paths_failed).max(1) as f64
    );
    if let Some(max) = max_despawns
        && s.despawned_stuck > max
    {
        println!(
            "FAIL      {} stuck despawns > --max-despawns {max}",
            s.despawned_stuck
        );
        std::process::exit(1);
    }
}

/// A radial town of about 3,600 junctions, drawn one street at a time. Ring `r` has `6r`
/// junctions, so ring streets stay about the same length as the town grows, and each junction
/// has a spoke to the nearest junction on the ring inside it. Some ring streets are left out.
fn organic_town(cfg: Config) -> World {
    const RINGS: u32 = 34;
    const SPACING: f32 = 110.0;
    const DROP_CHANCE: f32 = 0.12;
    let mut rng = Rng::new(cfg.seed ^ 0x5eed);
    let seed = cfg.seed;
    let mut world = World::new(cfg, RoadNetwork::default(), Rng::new(seed));

    // ring[r][i] = position of junction i on ring r; ring 0 is the centre.
    let mut ring: Vec<Vec<(f32, f32)>> = vec![vec![(0.0, 0.0)]];
    for r in 1..=RINGS {
        let n = 6 * r;
        let turn = rng.range(0.0, TAU);
        let points = (0..n)
            .map(|i| {
                let angle = turn + TAU * (i as f32 + rng.range(-0.2, 0.2)) / n as f32;
                let radius = SPACING * (r as f32 + rng.range(-0.15, 0.15));
                (radius * angle.cos(), radius * angle.sin())
            })
            .collect();
        ring.push(points);
    }
    let mut build = |(ax, ay): (f32, f32), (bx, by): (f32, f32)| {
        world.apply(Command::BuildRoad {
            ax,
            ay,
            bx,
            by,
            class: RoadClass::Street,
        });
    };
    for r in 1..ring.len() {
        let n = ring[r].len();
        for i in 0..n {
            let here = ring[r][i];
            if rng.unit() > DROP_CHANCE {
                build(here, ring[r][(i + 1) % n]);
            }
            let inner = ring[r - 1]
                .iter()
                .copied()
                .min_by(|a, b| {
                    let d = |p: &(f32, f32)| (p.0 - here.0).hypot(p.1 - here.1);
                    d(a).total_cmp(&d(b))
                })
                .expect("inner ring is never empty");
            build(here, inner);
        }
    }
    world
}
