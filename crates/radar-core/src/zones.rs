//! Zone definitions and default layout.
use crate::Point;
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ZoneKind {
    Detection,
    Ignore,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Zone {
    pub id: String,
    pub name: String,
    pub kind: ZoneKind,
    pub points: Vec<Point>,
}

/// Builds the initial detection sectors and settlement ignore zone.
pub fn default_zones() -> Vec<Zone> {
    let sector = |start: f64, end: f64| {
        let mut p = vec![Point { x: 0.0, y: 0.0 }];
        for i in 0..=48 {
            let a = start + (end - start) * i as f64 / 48.0;
            p.push(Point {
                x: 7000.0 * a.cos(),
                y: 7000.0 * a.sin(),
            });
        }
        p
    };
    vec![
        Zone {
            id: "north".into(),
            name: "Север".into(),
            kind: ZoneKind::Detection,
            points: sector(0.0, TAU / 2.0),
        },
        Zone {
            id: "south".into(),
            name: "Юг".into(),
            kind: ZoneKind::Detection,
            points: sector(TAU / 2.0, TAU),
        },
        Zone {
            id: "town".into(),
            name: "Населённый пункт".into(),
            kind: ZoneKind::Ignore,
            points: vec![
                Point {
                    x: -1200.0,
                    y: 2200.0,
                },
                Point {
                    x: -250.0,
                    y: 2000.0,
                },
                Point {
                    x: 1300.0,
                    y: -2000.0,
                },
                Point {
                    x: 400.0,
                    y: -2400.0,
                },
                Point {
                    x: -600.0,
                    y: 100.0,
                },
            ],
        },
    ]
}
