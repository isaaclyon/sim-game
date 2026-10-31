//! Headless traffic benchmark.
//!
//!     cargo run --release -p sim --example traffic_bench -- --agents 50000 --grid 60 --ticks 6000
//!
//! The browser runs 10 ticks per sim second, so a tick under 10 ms leaves room for 10x speed.

// Benchmarks measure wall time and report to stdout; neither feeds sim state.
#![allow(clippy::disallowed_methods, clippy::print_stdout)]

use std::time::Instant;

use sim::{Command, Config, World};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str, default: u32| -> u32 {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    };
    let agents = arg("--agents", 50_000);
    let grid = arg("--grid", 60);
    let ticks = arg("--ticks", 6_000);

    let cfg = Config::default();
    let dt = cfg.dt;
    let mut world = World::generate(cfg, grid, grid);
    world.apply(Command::SpawnAgents { count: agents });
    println!(
        "grid {grid}x{grid}: {} nodes, {} roads | {agents} agents | {ticks} ticks ({:.0} sim min)",
        world.roads.node_count(),
        world.roads.road_count(),
        ticks as f32 * dt / 60.0
    );

    let mut times = Vec::with_capacity(ticks as usize);
    let mut peak_driving = 0;
    let start = Instant::now();
    for i in 0..ticks {
        let t = Instant::now();
        world.tick();
        times.push(t.elapsed().as_secs_f64() * 1000.0);
        if i % 100 == 0 {
            peak_driving = peak_driving.max(world.stats().driving);
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
        "speed     {:.0}x realtime (sim {:.0}s in {wall:.1}s wall)",
        ticks as f64 * dt as f64 / wall,
        ticks as f64 * dt as f64
    );
    println!(
        "traffic   peak driving {peak_driving} | trips {} | paths ok {} failed {} | stuck despawns {}",
        s.trips_completed, s.paths_found, s.paths_failed, s.despawned_stuck
    );
}
