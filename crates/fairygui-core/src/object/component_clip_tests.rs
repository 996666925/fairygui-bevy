use super::*;
use crate::{
    geom::{Margin, Size},
    package::ObjectType,
    scroll::ScrollPane,
};

#[test]
fn hard_clip_bounds_and_hidden_api_coordinates_match_native_scalar_cases() {
    let (mut panes, mut hidden, mut coordinates) = (0, 0, 0);
    for line in include_str!("../../tests/fixtures/component-clip-reference.txt").lines() {
        if line.starts_with('#') {
            continue;
        }
        let (command, expected) = line.split_once(':').unwrap();
        let words: Vec<_> = command.split_whitespace().collect();
        let values: Vec<f32> = words[1..].iter().map(|s| s.parse().unwrap()).collect();
        let margin = Margin::new(values[0], values[1], values[2], values[3]);
        let mut store = ObjectStore::new();
        let id = store.create(ObjectType::Component);
        store.set_size(id, 120., 80.);
        let component = store.objects[id].control.component_mut().unwrap();
        component.margin = margin;
        component.overflow = OverflowType::Hidden;
        if words[0] == "pane" {
            let mut pane = ScrollPane::default();
            pane.clip = values[4] == 1.;
            pane.display_on_left = values[5] == 1.;
            pane.floating = values[6] == 1.;
            pane.dont_clip_margin = values[7] == 1.;
            pane.display_in_demand = values[8] == 1.;
            pane.set_content_size(if values[10] == 1. {
                Size::new(20., 20.)
            } else {
                Size::new(300., 200.)
            });
            component.scroll_pane = Some(pane);
            let bars = values[9] as u8;
            for (bit, size) in [(1, Size::new(8., 60.)), (2, Size::new(80., 6.))] {
                if bars & bit == 0 {
                    continue;
                }
                let bar = store.create(ObjectType::ScrollBar);
                store.set_size(bar, size.width, size.height);
                store.set_parent(bar, Some(id));
                let pane = store.objects[id]
                    .control
                    .component_mut()
                    .unwrap()
                    .scroll_pane
                    .as_mut()
                    .unwrap();
                if bit == 1 {
                    pane.vt_bar = Some(bar);
                } else {
                    pane.hz_bar = Some(bar);
                }
            }
            store.layout_scroll_bars(id);
            panes += 1;
        } else if words[0] == "api" {
            store.set_xy(id, 30., 70.);
            store.set_scale(id, values[4], values[5]);
            store.set_rotation(id, values[6]);
            store.set_pivot(id, 0.2, 0.3, values[7] == 1.);
            let point = Point::new(7., -3.);
            let global = store.local_to_global(id, point).unwrap();
            let local = store.global_to_local(id, global).unwrap();
            let actual = [global.x, global.y, local.x, local.y];
            for (actual, expected) in actual.into_iter().zip(expected.split_whitespace()) {
                let expected: f32 = expected.parse().unwrap();
                assert!(
                    (actual - expected).abs() < 0.0001,
                    "{line}: {actual} != {expected}"
                );
            }
            coordinates += 1;
            continue;
        } else {
            hidden += 1;
        }
        let actual = store.component_clip_rect(id);
        if expected.trim() == "none" {
            assert_eq!(actual, None, "{line}");
        } else {
            let actual = actual.unwrap();
            let actual = [actual.x, actual.y, actual.width, actual.height];
            let expected: Vec<f32> = expected
                .split_whitespace()
                .map(|s| s.parse().unwrap())
                .collect();
            assert_eq!(actual.as_slice(), expected, "{line}");
        }
    }
    assert_eq!((panes, hidden, coordinates), (1024, 4, 72));
}
