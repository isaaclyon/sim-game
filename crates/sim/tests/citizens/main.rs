use sim::{Command, Config, World, Zone};

fn town() -> World {
    let mut w = World::new(
        Config::default(),
        sim::road::RoadNetwork::generate_lattice(3, 3, 120.0),
        sim::rng::Rng::new(1),
    );
    w.apply(Command::FillBlock {
        x: 50.0,
        y: 50.0,
        zone: Zone::ResidentialLow,
    });
    w
}
fn advance(w: &mut World, seconds: u32) {
    for _ in 0..seconds * 10 {
        w.tick();
    }
}

#[test]
fn residents_arrive_without_jobs_and_population_is_not_cars() {
    let mut w = town();
    assert_eq!(w.stats().population, 0);
    advance(&mut w, 120);
    assert!(w.stats().population > 0);
    assert_eq!(w.stats().driving, 0);
    assert_eq!(
        w.stats().population as usize,
        w.citizens.active.iter().filter(|&&v| v).count()
    );
    for (h, &capacity) in w.homes.capacity.iter().enumerate() {
        assert!((2..=7).contains(&capacity));
        let household = w.homes.household[h].unwrap() as usize;
        assert!(w.households.members[household].len() <= capacity as usize);
    }
}

#[test]
fn normal_world_has_no_synthetic_traffic() {
    let mut w = World::generate(Config::default(), 3, 3);
    advance(&mut w, 120);
    assert_eq!(w.agents.len(), 0);
    assert_eq!(w.stats().population, 0);
    assert!(w.apply(Command::SpawnAgents { count: 20 }));
    advance(&mut w, 120);
    assert_eq!(w.agents.len(), 20);
    assert_eq!(w.stats().population, 0);
}

#[test]
fn repaint_preserves_residents_but_blocks_new_households() {
    let mut w = town();
    advance(&mut w, 20);
    let population = w.stats().population;
    w.apply(Command::FillBlock {
        x: 50.0,
        y: 50.0,
        zone: Zone::CommercialLow,
    });
    advance(&mut w, 120);
    assert_eq!(w.stats().population, population);
    w.apply(Command::FillBlock {
        x: 50.0,
        y: 50.0,
        zone: Zone::ResidentialLow,
    });
    advance(&mut w, 120);
    assert!(w.stats().population > population);
}

#[test]
fn road_demolition_reconciles_population_and_every_normal_car_has_an_owner() {
    let mut w = town();
    w.apply(Command::FillBlock {
        x: 50.0,
        y: 170.0,
        zone: Zone::CommercialLow,
    });
    advance(&mut w, 180);
    assert!(w.stats().population > 0);
    assert!(!w.agents.is_empty());
    let roads: Vec<_> = w.roads.active_roads().collect();
    for r in roads {
        let (x, y) = w
            .roads
            .lane_point(r * 2, w.roads.lane_len[r as usize * 2] / 2.0, 0.0);
        w.apply(Command::DeleteRoad { x, y });
        assert_eq!(
            w.stats().population as usize,
            w.citizens.active.iter().filter(|&&v| v).count()
        );
        for (c, &active) in w.citizens.active.iter().enumerate() {
            if active {
                assert!(w.buildings.active[w.citizens.location[c] as usize]);
                if let Some(b) = w.citizens.destination[c] {
                    assert!(w.buildings.active[b as usize]);
                }
            }
        }
    }
    assert_eq!(w.stats().population, 0);
    assert!(w.agents.citizen.iter().all(Option::is_some));
    // Drain more than a full day of old work, shopping and search deadlines.
    advance(&mut w, 14_401);
    assert_eq!(w.stats().driving, 0);
    assert_eq!(w.stats().entering, 0);
    assert_eq!(w.stats().awaiting_path, 0);
    assert_eq!(w.stats().population, 0);
}

#[test]
fn resident_state_is_deterministic() {
    let (mut a, mut b) = (town(), town());
    advance(&mut a, 180);
    advance(&mut b, 180);
    assert_eq!(a.state_hash(), b.state_hash());
    b.citizens.job[0] = Some(0);
    assert_ne!(a.state_hash(), b.state_hash());
}
