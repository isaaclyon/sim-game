use sim::rng::Rng;
use sim::road::{RoadClass, RoadNetwork};
use sim::{Command, Config, World};

fn empty() -> World {
    World::new(Config::default(), RoadNetwork::default(), Rng::new(1))
}
fn curve() -> Command {
    Command::BuildCurve {
        ax: 0.0,
        ay: 0.0,
        cx: 100.0,
        cy: 100.0,
        bx: 200.0,
        by: 0.0,
        class: RoadClass::Street,
    }
}
fn close(a: (f32, f32), b: (f32, f32)) {
    assert!((a.0 - b.0).hypot(a.1 - b.1) < 0.02, "{a:?} != {b:?}");
}

mod crossings;
mod editing;
mod queries;
