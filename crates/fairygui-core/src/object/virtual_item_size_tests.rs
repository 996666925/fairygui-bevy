use super::tests::{button, fixture};
use super::*;

#[test]
fn virtual_item_size_matches_native_baselines_layout_and_retained_record_dimensions() {
    let mut current = None;
    let (mut cases, mut snapshots) = (0, 0);
    for line in include_str!("../../tests/fixtures/virtual-item-size-reference.txt").lines() {
        let values: Vec<_> = line.split_whitespace().collect();
        if values.is_empty() || values[0].starts_with('#') {
            continue;
        }
        if values[0] == "case" {
            let layout = ListLayoutType::from_code(values[2].parse().unwrap());
            let (mut store, _, list) = fixture(layout, values[3] == "1", values[4] == "1");
            store
                .enable_virtual_list(list, "a", &mut |store, _| Some(button(store, 24.25, 19.5)))
                .unwrap();
            current = Some((store, list, layout));
            cases += 1;
            continue;
        }
        let (store, list, layout) = current.as_mut().unwrap();
        let list = *list;
        let a: f32 = values[2].parse().unwrap();
        let b: f32 = values[3].parse().unwrap();
        match values[1] {
            "size" => store.set_virtual_item_size(list, Size::new(a, b)).unwrap(),
            "count" => store.set_virtual_item_count(list, a as usize).unwrap(),
            _ => panic!("{line}"),
        }
        let vertical = matches!(
            layout,
            ListLayoutType::SingleColumn | ListLayoutType::FlowHorizontal
        );
        store
            .refresh_virtual_list(
                list,
                &mut |store, _| Some(button(store, 24.25, 19.5)),
                &mut |_| None,
                &mut |store, i, row| {
                    let old = store.object(row).unwrap().size();
                    store.set_size(
                        row,
                        if vertical {
                            old.width
                        } else {
                            22.25 + (i % 4) as f32 * 3.
                        },
                        if vertical {
                            13.5 + (i % 5) as f32 * 4.
                        } else {
                            old.height
                        },
                    );
                },
            )
            .unwrap();
        let state = &store.virtual_states[&list];
        let geometry = &state.geometry;
        let pane = store.scroll_pane(list).unwrap();
        let mut actual = vec![
            geometry.item_size.width,
            geometry.item_size.height,
            geometry.line_items as f32,
            geometry.page_rows as f32,
            geometry.sizes.len() as f32,
            state.num_items as f32,
            pane.content_size().width,
            pane.content_size().height,
            pane.scroll_step(),
            store.list_item_count(list) as f32,
        ];
        for index in [0, 50, 99, 100, 124, 125, 149] {
            let size = geometry
                .sizes
                .get(index)
                .copied()
                .unwrap_or(Size::new(-1., -1.));
            actual.extend([size.width, size.height]);
        }
        assert_eq!(actual.len(), values.len() - 4);
        for (actual, expected) in actual.into_iter().zip(&values[4..]) {
            let expected: f32 = expected.parse().unwrap();
            assert!(
                (actual - expected).abs() < 0.02,
                "case {}: {actual} != {expected}: {line}",
                cases - 1
            );
        }
        assert_eq!(
            store.virtual_list_info(list).unwrap().item_size,
            store.virtual_item_size(list).unwrap()
        );
        snapshots += 1;
    }
    assert_eq!((cases, snapshots), (20, 180));
}

#[test]
fn virtual_item_size_rejects_invalid_geometry_and_preserves_loop_records_and_scroll_step() {
    let (mut store, _, list) = fixture(ListLayoutType::SingleColumn, false, false);
    assert_eq!(store.virtual_item_size(list), None);
    assert_eq!(
        store.set_virtual_item_size(list, Size::new(40., 30.)),
        Err(VirtualListError::MissingPane)
    );
    store
        .enable_loop_virtual_list(list, "a", &mut |store, _| Some(button(store, 24., 20.)))
        .unwrap();
    store
        .set_virtual_item_size(list, Size::new(40., 30.))
        .unwrap();
    store.set_virtual_item_count(list, 2).unwrap();
    assert_eq!(
        store.virtual_states[&list].geometry.sizes,
        vec![Size::new(40., 30.); 12]
    );
    for size in [
        Size::new(0., 30.),
        Size::new(40., -2.),
        Size::new(f32::NAN, 30.),
        Size::new(40., f32::INFINITY),
    ] {
        assert_eq!(
            store.set_virtual_item_size(list, size),
            Err(VirtualListError::InvalidGeometry)
        );
        assert_eq!(store.virtual_item_size(list), Some(Size::new(40., 30.)));
    }
    // A negative gap accepted for the old baseline may reject a smaller size.
    let mut settings = store.list_layout_settings(list).unwrap();
    settings.line_gap = -25;
    assert!(store.set_list_layout_settings(list, settings));
    assert_eq!(
        store.set_virtual_item_size(list, Size::new(40., 25.)),
        Err(VirtualListError::InvalidGeometry)
    );
    store
        .set_virtual_item_size(list, Size::new(50., 40.))
        .unwrap();
    store.set_virtual_item_count(list, 3).unwrap();
    let sizes = &store.virtual_states[&list].geometry.sizes;
    assert_eq!(&sizes[..12], &[Size::new(40., 30.); 12]);
    assert_eq!(&sizes[12..], &[Size::new(50., 40.); 6]);
    assert_eq!(store.scroll_pane(list).unwrap().scroll_step(), 20.);
    store.remove(list);
    assert_eq!(store.virtual_item_size(list), None);
}
