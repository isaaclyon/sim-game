use super::*;

fn grid(cols: u32, rows: u32) -> Pollution {
    let zones = ZoneGrid::covering([0.0, 0.0, 0.0, 0.0]);
    let mut p = Pollution::covering(&zones);
    p.cols = cols;
    p.rows = rows;
    let n = (cols * rows) as usize;
    p.ground = vec![0; n];
    p.air = vec![0; n];
    p.noise = vec![0; n];
    p.land = vec![255; n];
    p
}

#[test]
fn the_grid_lines_up_with_the_zone_grid() {
    let zones = ZoneGrid::covering([0.0, 0.0, 100.0, 50.0]);
    let p = Pollution::covering(&zones);
    assert_eq!((p.x0, p.y0), (zones.x0, zones.y0));
    assert_eq!(p.cols, zones.cols.div_ceil(4));
    // Zone cell (5, 9) sits in pollution cell (1, 2).
    let index = 9 * zones.cols + 5;
    assert_eq!(
        p.cell_of_zone_cell(index, zones.cols),
        2 * p.cols as usize + 1
    );
    let (x, y) = zones.centre(index);
    assert_eq!(p.cell_at(x, y), Some(2 * p.cols as usize + 1));
}

#[test]
fn spreading_falls_off_with_distance_and_stops_at_the_radius() {
    let mut src = vec![0.0; 81];
    src[4 * 9 + 4] = 1.0;
    let out = field::spread(&src, 9, 9, 2);
    assert_eq!(out[4 * 9 + 4], 1.0);
    assert!(out[4 * 9 + 5] < 1.0 && out[4 * 9 + 5] > out[4 * 9 + 6]);
    assert!(out[4 * 9 + 6] > 0.0);
    assert_eq!(out[4 * 9 + 7], 0.0);
    assert_eq!(
        out[4 * 9 + 3],
        out[3 * 9 + 4],
        "the same in every direction"
    );
}

#[test]
fn ground_rises_under_sources_and_fades_slowly_after() {
    let mut p = grid(3, 1);
    let mut src = p.sources();
    src.ground = vec![
        GroundSource::Under,
        GroundSource::Beside,
        GroundSource::None,
    ];
    let mut rises = 0;
    while p.ground[0] < GROUND_MAX {
        p.update(&src);
        rises += 1;
    }
    assert_eq!(p.ground[1], GROUND_MAX / 2, "beside caps at half");
    assert_eq!(p.ground[2], 0);
    let clean = p.sources();
    let mut falls = 0;
    while p.ground[0] > 0 {
        p.update(&clean);
        falls += 1;
    }
    assert!(
        falls > 4 * rises,
        "fades much slower ({falls}) than it rises ({rises})"
    );
}

#[test]
fn pollution_lowers_land_value_and_bumps_the_version() {
    let mut p = grid(1, 1);
    p.update(&p.sources());
    assert_eq!(
        (p.land[0], p.version),
        (255, 0),
        "a clean update changes nothing"
    );
    let mut src = p.sources();
    src.air[0] = AIR_FULL;
    p.update(&src);
    assert_eq!(p.air[0], 255);
    assert_eq!(p.version, 1);
    assert!(p.land[0] < 255 && p.land[0] > 0);
    src.noise[0] = NOISE_FULL * 10.0;
    p.update(&src);
    assert_eq!(p.noise[0], 255, "levels saturate");
    assert_eq!(p.land[0], 0, "air and noise at full together leave nothing");
}

#[test]
fn land_value_of_a_lot_is_the_mean_of_its_cells() {
    let mut p = grid(2, 1);
    p.land = vec![255, 55];
    assert_eq!(p.land_of([0, 1, 1].into_iter()), 121);
    assert_eq!(p.land_of(std::iter::empty()), 255);
}
