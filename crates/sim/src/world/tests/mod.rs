mod editing;
mod traffic;

use super::*;
use crate::road::{NodeId, RoadNetwork};

fn small_world(seed: u64) -> World {
    let cfg = Config {
        seed,
        ..Config::default()
    };
    let mut world = World::generate(cfg, 12, 12);
    world.apply(Command::SpawnAgents { count: 2_000 });
    world
}

/// A world with one straight two-way road per `(ax, ay, bx, by)`, each with its own nodes.
fn roads(segments: &[(f32, f32, f32, f32)]) -> World {
    let mut net = RoadNetwork::default();
    for &(ax, ay, bx, by) in segments {
        let (a, b) = (net.add_node(ax, ay), net.add_node(bx, by));
        net.add_road(a, b, RoadClass::Street);
    }
    World::new(Config::default(), net, Rng::new(1))
}

fn build(world: &mut World, ax: f32, ay: f32, bx: f32, by: f32) -> bool {
    world.apply(Command::BuildRoad {
        ax,
        ay,
        bx,
        by,
        class: RoadClass::Street,
    })
}

fn node_at(world: &World, x: f32, y: f32) -> NodeId {
    world
        .roads
        .nearest_node(x, y, 0.5)
        .unwrap_or_else(|| panic!("no node at ({x}, {y})"))
}

/// A four-way junction at the origin with 100 m arms east, north, west and south. Arm `k`
/// owns lane `2k` (out of the junction) and `2k + 1` (into it).
fn crossroads(ew: RoadClass) -> World {
    let mut net = RoadNetwork::default();
    let centre = net.add_node(0.0, 0.0);
    for k in 0..4 {
        let angle = k as f32 * std::f32::consts::FRAC_PI_2;
        let arm = net.add_node(100.0 * angle.cos(), 100.0 * angle.sin());
        net.add_road(centre, arm, if k % 2 == 0 { ew } else { RoadClass::Street });
    }
    World::new(Config::default(), net, Rng::new(1))
}

/// Puts a car at the stop line of `from`, about to drive on to `to`, having waited `waited`
/// ticks already.
fn wait_at_line(world: &mut World, from: LaneId, to: LaneId, waited: u16) -> AgentId {
    let agent = world.agents.push(world.roads.lane_from[from as usize]);
    let a = agent as usize;
    world.agents.dest[a] = world.roads.lane_to[to as usize];
    world.agents.path[a] = vec![from, to];
    world.traffic.enter(from, agent, &mut world.agents);
    world.agents.pos[a] = world.junctions.stop[from as usize];
    world.agents.stuck_ticks[a] = waited;
    agent
}
