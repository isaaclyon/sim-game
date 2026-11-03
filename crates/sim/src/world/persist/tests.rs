//! Saves that pass the envelope checks but hold inconsistent state are refused.
use super::*;
use crate::agents::AgentState;
use crate::road::RoadClass;
use crate::{Command, Config, Zone};
use std::cmp::Reverse;

fn town() -> World {
    let mut w = World::generate(Config::default(), 4, 4);
    w.apply(Command::SpawnAgents { count: 30 });
    for _ in 0..100 {
        w.tick();
    }
    w
}

/// Homes, shops and industry with residents, cars and walkers.
fn zoned_town() -> World {
    let mut w = World::new(
        Config {
            ignore_demand: true,
            ..Config::default()
        },
        crate::road::RoadNetwork::generate_lattice(5, 3, 120.0),
        crate::rng::Rng::new(3),
    );
    for (x, zone) in [
        (50.0, Zone::ResidentialLow),
        (170.0, Zone::CommercialLow),
        (290.0, Zone::IndustrialLow),
        (410.0, Zone::ResidentialHigh),
    ] {
        assert!(w.apply(Command::FillBlock { x, y: 50.0, zone }));
    }
    w.apply(Command::SpawnAgents { count: 100 });
    for _ in 0..1500 {
        w.tick();
    }
    assert!(w.residents.employed > 0);
    w
}

fn refused(w: &World) -> SaveError {
    World::load(&w.save().unwrap())
        .err()
        .expect("inconsistent save loaded")
}

/// Re-signs `payload` so only decoding and validation can reject it.
fn signed(payload: &[u8]) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    out.extend_from_slice(&SAVE_VERSION.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    out.extend_from_slice(&checksum(payload).to_le_bytes());
    out.extend_from_slice(payload);
    out
}

#[test]
fn a_consistent_town_loads() {
    let w = town();
    assert!(w.validate().is_ok());
    assert!(World::load(&w.save().unwrap()).is_ok());
}

#[test]
fn dangling_ids_are_refused() {
    let mut w = town();
    w.agents.path[0].push(u32::MAX);
    assert_eq!(refused(&w), SaveError::Invalid("agent routes"));

    let mut w = town();
    w.traffic.queues[0].push_back(w.agents.len() as u32);
    assert_eq!(refused(&w), SaveError::Invalid("queued agents"));

    let mut w = town();
    w.wake.push(Reverse((0, 1_000_000)));
    assert_eq!(refused(&w), SaveError::Invalid("parked agents"));

    let mut w = town();
    w.roads.lane_to[0] = w.roads.node_count() as u32;
    assert_eq!(refused(&w), SaveError::Invalid("lane ends"));
}

#[test]
fn mismatched_lengths_are_refused() {
    let mut w = town();
    w.agents.pos.pop();
    assert_eq!(refused(&w), SaveError::Invalid("agent fields"));

    let mut w = town();
    w.zones.cells.pop();
    assert_eq!(refused(&w), SaveError::Invalid("zone grid size"));

    let mut w = town();
    w.cfg.dt = 0.0;
    assert_eq!(refused(&w), SaveError::Invalid("tick length"));
}

#[test]
fn a_signed_but_undecodable_payload_is_refused() {
    let save = town().save().unwrap();
    let payload = &save[HEADER..];
    assert!(World::load(&signed(payload)).is_ok());
    assert!(matches!(
        World::load(&signed(&payload[..payload.len() / 2])),
        Err(SaveError::Invalid(_))
    ));
    let mut longer = payload.to_vec();
    longer.push(0);
    assert_eq!(
        World::load(&signed(&longer)).err(),
        Some(SaveError::Invalid("payload has trailing bytes"))
    );
    // Arbitrary bytes do not decode as a world.
    assert!(World::load(&signed(&[0xff; 64])).is_err());
}

#[test]
fn length_and_checksum_mismatches_are_corrupt() {
    let save = town().save().unwrap();
    let mut flipped = save.clone();
    *flipped.last_mut().unwrap() ^= 1;
    assert_eq!(World::load(&flipped).err(), Some(SaveError::Corrupt));
    assert_eq!(
        World::load(&save[..save.len() - 1]).err(),
        Some(SaveError::Truncated)
    );
    assert_eq!(
        World::load(&save[..HEADER - 1]).err(),
        Some(SaveError::Truncated)
    );
}

#[test]
fn live_worlds_pass_validation_through_traffic_closures_and_edits() {
    let mut w = zoned_town();
    let edits = [
        Command::FillBlock {
            x: 170.0,
            y: 50.0,
            zone: Zone::ResidentialLow,
        },
        Command::BuildRoad {
            ax: 0.0,
            ay: 240.0,
            bx: 480.0,
            by: 360.0,
            class: RoadClass::Main,
        },
        Command::DeleteRoad { x: 360.0, y: 60.0 },
        Command::FillBlock {
            x: 290.0,
            y: 50.0,
            zone: Zone::None,
        },
    ];
    let mut seen = [false; 3];
    for cmd in edits {
        assert!(w.apply(cmd));
        assert_eq!(w.validate(), Ok(()), "right after an edit");
        for t in 0..1000 {
            w.tick();
            let s = w.stats();
            seen[0] |= s.driving > 0;
            seen[1] |= s.walking > 0;
            seen[2] |= s.entering > 0;
            if t % 5 == 0 {
                assert_eq!(w.validate(), Ok(()), "tick {}", w.tick_count());
            }
        }
    }
    assert_eq!(
        seen, [true; 3],
        "drivers, walkers and entering cars were checked"
    );
}

#[test]
fn non_finite_numbers_are_refused() {
    let mut w = town();
    w.cfg.dwell_max_s = f32::NAN;
    assert_eq!(refused(&w), SaveError::Invalid("dwell range"));

    let mut w = town();
    w.cfg.stuck_limit_s = f32::INFINITY;
    assert_eq!(refused(&w), SaveError::Invalid("config values"));

    let mut w = town();
    w.roads.node_x[0] = f32::NAN;
    assert_eq!(refused(&w), SaveError::Invalid("node positions"));

    let mut w = town();
    w.roads.lane_speed[0] = 0.0;
    assert_eq!(refused(&w), SaveError::Invalid("lane lengths and speeds"));

    let mut w = town();
    w.agents.pos[0] = f32::NAN;
    assert_eq!(refused(&w), SaveError::Invalid("agent motion"));

    let mut w = zoned_town();
    w.citizens.walk_speed[0] = f32::INFINITY;
    assert_eq!(refused(&w), SaveError::Invalid("citizen walking"));
}

#[test]
fn travelling_agents_need_a_current_lane() {
    let mut w = town();
    let a = w
        .agents
        .state
        .iter()
        .position(|&s| s == AgentState::Driving)
        .unwrap();
    w.agents.path_idx[a] = w.agents.path[a].len() as u32;
    assert_eq!(refused(&w), SaveError::Invalid("travelling agent routes"));

    let mut w = town();
    let a = w
        .agents
        .state
        .iter()
        .position(|&s| s == AgentState::Parked)
        .unwrap();
    w.agents.state[a] = AgentState::Entering;
    w.agents.path[a].clear();
    assert_eq!(refused(&w), SaveError::Invalid("travelling agent routes"));

    let mut w = town();
    let a = w
        .agents
        .state
        .iter()
        .position(|&s| s == AgentState::Parked)
        .unwrap();
    w.agents.state[a] = AgentState::Walking;
    w.agents.path[a] = vec![0];
    w.agents.path_idx[a] = 0;
    assert_eq!(refused(&w), SaveError::Invalid("walker citizens"));
}

#[test]
fn drivers_and_lane_queues_must_match() {
    let mut w = town();
    let a = w
        .agents
        .state
        .iter()
        .position(|&s| s == AgentState::Driving)
        .unwrap();
    let lane = w.agents.lane[a] as usize;
    w.traffic.queues[lane].retain(|&q| q as usize != a);
    assert_eq!(refused(&w), SaveError::Invalid("unqueued drivers"));

    let mut w = town();
    let a = w
        .agents
        .state
        .iter()
        .position(|&s| s == AgentState::Parked)
        .unwrap();
    w.traffic.queues[0].push_back(a as u32);
    assert_eq!(refused(&w), SaveError::Invalid("lane queue drivers"));
}

#[test]
fn counters_that_would_underflow_are_refused() {
    let mut w = zoned_town();
    w.residents.employed = w.residents.population + 1;
    assert_eq!(refused(&w), SaveError::Invalid("employment"));

    let mut w = zoned_town();
    w.residents.population -= 1;
    assert_eq!(refused(&w), SaveError::Invalid("population"));

    let mut w = zoned_town();
    let b = w.residents.workers.iter().position(|&n| n > 0).unwrap();
    w.residents.workers[b] -= 1;
    assert_eq!(refused(&w), SaveError::Invalid("worker counts"));

    let mut w = zoned_town();
    let b = w.residents.workers.iter().position(|&n| n > 0).unwrap();
    w.residents.jobs[b] = w.residents.workers[b] - 1;
    assert_eq!(refused(&w), SaveError::Invalid("workers within jobs"));

    let mut w = zoned_town();
    w.supply.commercial_jobs += 1;
    assert_eq!(refused(&w), SaveError::Invalid("job supply"));
}
