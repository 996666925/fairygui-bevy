use super::*;
use crate::{package::ObjectType, widget::ControlData};

#[test]
fn native_progress_updates_match_both_axes_fill_reverse_resize_ani_and_titles() {
    let mut store = ObjectStore::new();
    let mut owner = None;
    let mut bars = None;
    for line in include_str!("../../tests/fixtures/progress-reference.txt")
        .lines()
        .filter(|line| !line.starts_with('#'))
    {
        if let Some(input) = line.strip_prefix("case ") {
            let input: Vec<i32> = input
                .split_whitespace()
                .map(|n| n.parse().unwrap())
                .collect();
            store = ObjectStore::new();
            let id = store.create(ObjectType::ProgressBar);
            store.set_size(id, 300.0, 180.0);
            if let ControlData::ProgressBar(data) = &mut store.object_mut(id).unwrap().control {
                data.min = 10.;
                data.max = 110.;
                data.reverse = input[1] != 0;
                data.title_type = input[2];
                data.value = 50.;
            }
            let kind = match input[0] {
                0 => ObjectType::Graph,
                1 => ObjectType::Image,
                _ => ObjectType::Loader,
            };
            let h = store.create(kind);
            let v = store.create(kind);
            for (bar, name, w, height) in [(h, "bar", 260.0, 20.0), (v, "bar_v", 20.0, 120.0)] {
                store.set_parent(bar, Some(id));
                store.set_name(bar, name);
                store.set_xy(bar, 12.0, 8.0);
                store.set_size(bar, w, height);
                store.set_image_fill(bar, 5, 0, true, 0.3);
            }
            let title = store.create(ObjectType::Text);
            store.set_parent(title, Some(id));
            store.set_name(title, "title");
            let ani = store.create(ObjectType::MovieClip);
            store.set_parent(ani, Some(id));
            store.set_name(ani, "ani");
            // Native construction captures these dimensions before its first update.
            store.remember_progress_bars(id);
            owner = Some(id);
            bars = Some((h, v, title, ani));
        } else {
            let (input, expected) = line
                .strip_prefix("layout ")
                .unwrap()
                .split_once(" | ")
                .unwrap();
            let input: Vec<i32> = input
                .split_whitespace()
                .map(|n| n.parse().unwrap())
                .collect();
            let id = owner.unwrap();
            let (h, v, title, ani) = bars.unwrap();
            store.set_size(id, input[1] as f32, input[2] as f32);
            store.set_progress(id, input[0]);
            let expected: Vec<_> = expected.split_whitespace().collect();
            for (bar, offset) in [(h, 0), (v, 5)] {
                let object = store.object(bar).unwrap();
                let amount = match object.control() {
                    ControlData::Image(d) => d.fill_amount,
                    ControlData::Loader(d) => d.fill_amount,
                    _ => 0.3,
                };
                for (index, actual) in [
                    f64::from(object.x()),
                    f64::from(object.y()),
                    f64::from(object.width()),
                    f64::from(object.height()),
                    amount,
                ]
                .into_iter()
                .enumerate()
                {
                    let expected: f64 = expected[offset + index].parse().unwrap();
                    assert!(
                        (actual - expected).abs() < 0.0001,
                        "{line}: field {} {actual} != {expected}",
                        offset + index
                    );
                }
            }
            assert_eq!(
                store.animation_state(ani).unwrap().1,
                expected[10].parse::<i32>().unwrap(),
                "{line}"
            );
            let ControlData::Text(data) = store.object(title).unwrap().control() else {
                unreachable!()
            };
            assert_eq!(data.text, expected[11], "{line}");
        }
    }
}

#[test]
fn official_cooldown_initial_values_and_changes_drive_fill_without_resizing_the_bar() {
    let package = crate::FuiPackage::parse(
        "Cooldown",
        include_bytes!("../../tests/fixtures/Cooldown.fui"),
    )
    .unwrap();
    let mut store = ObjectStore::new();
    let root = crate::widget::create_from_package(
        &mut store,
        &package,
        package.item_index_by_name("Main").unwrap(),
        &crate::package::LoadContext::none(),
    )
    .unwrap();
    for name in ["b0", "b1"] {
        let owner = store.child_by_name(root, name).unwrap();
        let bar = store.child_by_name(owner, "bar").unwrap();
        let size = store.object(bar).unwrap().size();
        let ControlData::Image(data) = store.object(bar).unwrap().control() else {
            panic!("image progress bar")
        };
        assert_eq!(data.fill_method, 5);
        for value in [0, 25, 100, 50] {
            store.set_progress(owner, value);
            let object = store.object(bar).unwrap();
            assert_eq!(object.size(), size);
            let ControlData::Image(data) = object.control() else {
                unreachable!()
            };
            let ControlData::ProgressBar(progress) = store.object(owner).unwrap().control() else {
                unreachable!()
            };
            let ratio =
                ((progress.value - progress.min) / (progress.max - progress.min)).clamp(0., 1.);
            assert!(
                (data.fill_amount - if progress.reverse { 1.0 - ratio } else { ratio }).abs()
                    < 0.00001
            );
        }
    }
}
