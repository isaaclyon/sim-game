//! Times grid tool previews (ADR 0049), which run on every pointer move while stretching.
//!
//! `cargo run --release -p sim --example grid_bench`
//!
//! Prints ms per preview for the largest grid, 20 × 20 blocks (840 edges), on empty land and
//! dragged over the edge of the default benchmark city.
// Wall-clock timing only; nothing here feeds sim state.
#![allow(clippy::disallowed_methods, clippy::print_stdout)]

use std::time::Instant;

use sim::road::RoadClass;
use sim::{Command, Config, World};

fn time(name: &str, world: &World, cmd: &Command) {
    let runs = 5;
    let t = Instant::now();
    let mut legs = 0;
    for _ in 0..runs {
        legs = world.grid_preview(cmd).map_or(0, |p| p.roads.len());
    }
    let ms = t.elapsed().as_secs_f64() * 1000.0 / runs as f64;
    println!(
        "{name}: {ms:.1} ms per preview, {legs} legs, {} roads in the city",
        world.roads.road_count()
    );
}

fn main() {
    let grid = |ox: f32, oy: f32| Command::BuildGrid {
        ox,
        oy,
        ux: 1.0,
        uy: 0.0,
        cell: 80.0,
        cols: 20,
        rows: 20,
        class: RoadClass::Street,
    };
    let empty = World::empty(Config::default(), 30, 30);
    time("20x20 on empty land", &empty, &grid(0.0, 0.0));
    let town = World::generate(Config::default(), 20, 20);
    let [_, _, x1, _] = town.roads.bounds();
    time(
        "20x20 over a 20x20 town's edge",
        &town,
        &grid(x1 - 400.0, -100.0),
    );
}
