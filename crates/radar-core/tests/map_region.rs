use radar_core::{Point, ORIGIN_LAT, ORIGIN_LON, RADAR_RANGE_M};

#[test]
fn rust_origin_matches_map_region_and_scan_fits_offline_coverage() {
    let region: serde_json::Value =
        serde_json::from_str(include_str!("../../../map-region.json")).unwrap();
    assert_eq!(region["center"][0].as_f64().unwrap(), ORIGIN_LON);
    assert_eq!(region["center"][1].as_f64().unwrap(), ORIGIN_LAT);
    let bbox: Vec<f64> = region["bbox"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_f64().unwrap())
        .collect();
    for i in 0..360 {
        let angle = (i as f64).to_radians();
        let [lng, lat] = Point {
            x: RADAR_RANGE_M * angle.cos(),
            y: RADAR_RANGE_M * angle.sin(),
        }
        .geographic();
        assert!((bbox[0]..=bbox[2]).contains(&lng));
        assert!((bbox[1]..=bbox[3]).contains(&lat));
    }
}
