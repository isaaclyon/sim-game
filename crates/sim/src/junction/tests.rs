use super::*;
/// A junction at the origin with arms 100 m away, east, north, west, south in that order,
/// skipping any arm whose class is `None`. Arm `k` owns lane `2k` (out of the junction)
/// and `2k + 1` (into it).
fn junction(arms: [Option<RoadClass>; 4]) -> (RoadNetwork, Junctions) {
    let mut net = RoadNetwork::default();
    let centre = net.add_node(0.0, 0.0);
    for (k, class) in arms.iter().enumerate() {
        let Some(class) = *class else { continue };
        let angle = k as f32 * PI / 2.0;
        let arm = net.add_node(100.0 * angle.cos(), 100.0 * angle.sin());
        net.add_road(centre, arm, class);
    }
    net.rebuild_adjacency();
    let mut j = Junctions::default();
    j.sync(&net);
    (net, j)
}

const S: Option<RoadClass> = Some(RoadClass::Street);
const A: Option<RoadClass> = Some(RoadClass::Main);
// Lanes of the four-way junction.
const IN_E: LaneId = 1;
const IN_N: LaneId = 3;
const IN_W: LaneId = 5;
const IN_S: LaneId = 7;
const OUT_E: LaneId = 0;
const OUT_N: LaneId = 2;
const OUT_W: LaneId = 4;
const OUT_S: LaneId = 6;

#[test]
fn splitting_curve_preserves_reservations_at_the_old_far_end() {
    let mut net = RoadNetwork::default();
    let a = net.add_node(0.0, 0.0);
    let b = net.add_node(200.0, 0.0);
    let c = net.add_node(300.0, 0.0);
    net.add_curve(a, b, (100.0, 100.0), RoadClass::Street);
    net.add_road(b, c, RoadClass::Street);
    net.rebuild_adjacency();
    let mut j = Junctions::default();
    j.sync(&net);
    j.crossings[b as usize] = vec![
        Crossing {
            from: 0,
            to: 2,
            until: 50,
        },
        Crossing {
            from: 3,
            to: 1,
            until: 60,
        },
    ];
    let incoming = j.chord_in[0];
    let outgoing = j.chord_out[1];
    let mid = net.add_node(100.0, 50.0);
    let rest = net.split_road(0, mid);
    j.remap_split(0, rest);
    net.rebuild_adjacency();
    j.sync(&net);
    let crossings = &j.crossings[b as usize];
    assert_eq!(
        (crossings[0].from, crossings[0].to, crossings[0].until),
        (2 * rest, 2, 50)
    );
    assert_eq!(
        (crossings[1].from, crossings[1].to, crossings[1].until),
        (3, 2 * rest + 1, 60)
    );
    assert!((j.chord_in[2 * rest as usize] - incoming).abs() < 1e-5);
    assert!((j.chord_out[2 * rest as usize + 1] - outgoing).abs() < 1e-5);
}

#[test]
fn control_follows_shape() {
    assert_eq!(junction([S, S, S, S]).1.control[0], Control::Priority);
    assert_eq!(junction([A, S, A, S]).1.control[0], Control::Signal);
    assert_eq!(junction([A, None, A, S]).1.control[0], Control::Priority);
    assert_eq!(junction([S, None, S, None]).1.control[0], Control::None);
}

#[test]
fn turns_are_classified_for_right_hand_traffic() {
    let (_, j) = junction([S, S, S, S]);
    // Arriving from the east means driving west, so north is on the right.
    assert_eq!(j.turn(IN_E, OUT_W), Turn::Straight);
    assert_eq!(j.turn(IN_E, OUT_N), Turn::Right);
    assert_eq!(j.turn(IN_E, OUT_S), Turn::Left);
    assert_eq!(
        j.turn(IN_E, OUT_E),
        Turn::Left,
        "U-turns cross oncoming traffic"
    );
}

#[test]
fn conflicts_follow_crossing_paths() {
    let (_, j) = junction([S, S, S, S]);
    assert!(j.conflicts(IN_E, OUT_W, IN_N, OUT_S), "crossing straights");
    assert!(!j.conflicts(IN_E, OUT_W, IN_W, OUT_E), "opposite straights");
    assert!(
        !j.conflicts(IN_E, OUT_N, IN_W, OUT_S),
        "opposite right turns"
    );
    assert!(
        j.conflicts(IN_E, OUT_S, IN_W, OUT_E),
        "left across oncoming"
    );
    assert!(
        j.conflicts(IN_E, OUT_W, IN_N, OUT_W),
        "merging into one exit"
    );
    assert!(
        !j.conflicts(IN_E, OUT_W, IN_E, OUT_N),
        "same lane keeps queue order"
    );
    // Symmetric.
    assert!(j.conflicts(IN_N, OUT_S, IN_E, OUT_W));
}

#[test]
fn equal_roads_yield_to_the_right() {
    let (net, j) = junction([S, S, S, S]);
    let mine = |lane, next| (lane, j.turn(lane, next), Light::Uncontrolled);
    // Driving north from the south arm, the east arm is on the right.
    assert!(j.must_yield(&net, 0, mine(IN_S, OUT_N), (IN_E, OUT_W)));
    assert!(!j.must_yield(&net, 0, mine(IN_E, OUT_W), (IN_S, OUT_N)));
    // Left-turners give way to oncoming traffic.
    assert!(j.must_yield(&net, 0, mine(IN_E, OUT_S), (IN_W, OUT_E)));
    assert!(!j.must_yield(&net, 0, mine(IN_W, OUT_E), (IN_E, OUT_S)));
}

#[test]
fn side_road_yields_to_the_main_road() {
    // An avenue running east-west with a street joining from the south.
    let (net, j) = junction([A, None, A, S]);
    let (in_w, out_w, in_s) = (3, 2, 5);
    let mine = |lane, next| (lane, j.turn(lane, next), Light::Uncontrolled);
    assert!(j.must_yield(&net, 0, mine(in_s, out_w), (IN_E, out_w)));
    assert!(!j.must_yield(&net, 0, mine(IN_E, out_w), (in_s, out_w)));
    assert!(!j.must_yield(&net, 0, mine(in_w, OUT_E), (in_s, OUT_E)));
}

#[test]
fn signals_pair_opposite_approaches() {
    let (net, j) = junction([A, S, A, S]);
    assert_eq!(j.groups[0], 2);
    assert_eq!(j.group[IN_E as usize], j.group[IN_W as usize]);
    assert_eq!(j.group[IN_N as usize], j.group[IN_S as usize]);
    assert_ne!(j.group[IN_E as usize], j.group[IN_N as usize]);
    let mut out = Vec::new();
    j.write_signal_snapshot(&net, 6.0, &mut out);
    assert_eq!(out.len(), 4 * 4);
    let green = out.chunks(4).filter(|s| s[3] == 1.0).count();
    assert_eq!(green, 2, "one pair of approaches is green");
}
