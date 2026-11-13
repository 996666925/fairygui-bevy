//! 滚动容器的集成测试，使用官方示例包中的真实可滚动容器。

use fairygui_core::GObjectId;
use fairygui_core::object::ObjectStore;
use fairygui_core::package::{FuiPackage, LoadContext, PackageItemType, PackageRegistry};
use fairygui_core::scroll::{ScrollBarDisplayType, ScrollDefaults};
use fairygui_core::widget::{create_from_package, create_from_package_with_registry};
use std::sync::Arc;

const BASICS: &[u8] = include_bytes!("fixtures/Basics.fui");

/// 在包内所有组件中找出第一个可滚动容器。
fn find_scrollable(package: &FuiPackage) -> Option<(ObjectStore, GObjectId)> {
    for (index, item) in package.items().iter().enumerate() {
        if item.item_type != PackageItemType::Component {
            continue;
        }

        let mut store = ObjectStore::new();
        let Ok(root) = create_from_package(&mut store, package, index, &LoadContext::none()) else {
            continue;
        };

        let mut found = None;
        if store.scroll_pane(root).is_some() {
            found = Some(root);
        }

        let mut visit = |id: GObjectId| {
            if found.is_none() && store.scroll_pane(id).is_some() {
                found = Some(id);
            }
        };
        store.for_each_descendant(root, &mut visit);

        if let Some(id) = found {
            return Some((store, id));
        }
    }
    None
}

#[test]
fn sample_package_contains_scrollable_containers() {
    let package = FuiPackage::parse("Basics", BASICS).expect("parse basics package");
    assert!(
        find_scrollable(&package).is_some(),
        "官方示例包中应当包含可滚动容器"
    );
}

#[test]
fn package_registry_scroll_defaults_reach_nested_component_and_list_panes() {
    let package = FuiPackage::parse("Basics", BASICS).expect("parse basics package");
    let mut registry = PackageRegistry::default();
    registry.add(Arc::new(package.clone()), "ui/Basics");
    registry.set_scroll_defaults(ScrollDefaults {
        bar_display: ScrollBarDisplayType::Auto,
        touch_effect: false,
        bounceback_effect: false,
        scroll_step: 12.5,
        paging_threshold: 0.45,
    });

    let mut list_panes = 0;
    let mut component_panes = 0;
    for name in ["Demo_List", "Demo_Clip&Scroll"] {
        let index = package.item_index_by_name(name).unwrap();
        let mut store = ObjectStore::new();
        let context = registry.load_context(&package);
        let root =
            create_from_package_with_registry(&mut store, &package, index, &context, &registry)
                .unwrap();
        let mut objects = vec![root];
        store.for_each_descendant(root, &mut |id| objects.push(id));
        for object in objects {
            if let Some(pane) = store.scroll_pane(object) {
                assert_eq!(pane.scroll_step(), 12.5);
                assert_eq!(pane.paging_threshold(), 0.45);
                if matches!(
                    store.object(object).unwrap().control(),
                    fairygui_core::widget::ControlData::List(_)
                ) {
                    list_panes += 1;
                } else {
                    component_panes += 1;
                }
            }
        }
    }
    assert!(list_panes > 0, "inline lists receive the configuration");
    assert!(
        component_panes > 0,
        "nested component resources receive the configuration"
    );
}

/// 拖动内容本身来滚动，松手后惯性接管。
///
/// 在此之前核心层的拖拽与速度字段一直没有输入来源——惯性等于没有。
#[test]
fn dragging_content_drives_scrolling_and_inertia() {
    let package = FuiPackage::parse("Basics", BASICS).expect("parse basics package");
    let (mut store, id) = find_scrollable(&package).expect("示例包中应当包含可滚动容器");

    // 造出可滚动的范围，否则拖动会立刻被边界夹住。
    store.update_scroll_content_size(id, fairygui_core::geom::Size::new(2000.0, 2000.0));

    let before = store.scroll_pane(id).expect("pane").pos();

    assert!(store.begin_scroll_drag(id));
    // Native velocity uses filtered samples and a screen-speed threshold;
    // sustain the gesture instead of assuming one slow sample has inertia.
    for _ in 0..6 {
        assert!(store.drag_scroll_by(id, 0.0, -40.0, 1.0 / 60.0));
    }

    let after = store.scroll_pane(id).expect("pane").pos();
    assert!(
        after.y > before.y,
        "向上拖动内容会让视口下移（{before:?} -> {after:?}）"
    );

    // 松手：位置不再跟随，但速度保留下来，由惯性继续推进。
    assert!(store.end_scroll_drag(id));
    let resting = store.scroll_pane(id).expect("pane").pos();

    store.advance_scrolls(1.0 / 60.0);
    let sliding = store.scroll_pane(id).expect("pane").pos();
    assert!(
        sliding.y > resting.y,
        "松手后应当靠惯性继续滑动（{resting:?} -> {sliding:?}）"
    );
}

#[test]
fn scroll_position_is_clamped_to_content_range() {
    let package = FuiPackage::parse("Basics", BASICS).expect("parse basics package");
    let (mut store, id) = find_scrollable(&package).expect("示例包中应当包含可滚动容器");

    let max = store.scroll_pane(id).expect("scroll pane").max_scroll_pos();

    // 给一个远超范围的位置，应当被夹到边界上。
    store.set_scroll_pos(id, max.x + 5_000.0, max.y + 5_000.0, false);

    let pane = store.scroll_pane(id).expect("scroll pane");
    assert!(
        (pane.pos().x - max.x).abs() < 0.001,
        "横向应当停在边界 {}，实际 {}",
        max.x,
        pane.pos().x
    );
    assert!(
        (pane.pos().y - max.y).abs() < 0.001,
        "纵向应当停在边界 {}，实际 {}",
        max.y,
        pane.pos().y
    );
}

#[test]
fn scroll_direction_matches_description() {
    let package = FuiPackage::parse("Basics", BASICS).expect("parse basics package");
    let (store, id) = find_scrollable(&package).expect("示例包中应当包含可滚动容器");
    let pane = store.scroll_pane(id).expect("scroll pane");

    // 只能有一个方向被允许，或两个都允许；不该出现「都不允许」。
    assert!(
        pane.scroll_type.allows_horizontal() || pane.scroll_type.allows_vertical(),
        "滚动方向应当至少允许一个轴向"
    );
}

#[test]
fn inertia_advances_towards_boundary() {
    let package = FuiPackage::parse("Basics", BASICS).expect("parse basics package");
    let (mut store, id) = find_scrollable(&package).expect("示例包中应当包含可滚动容器");

    let max = store.scroll_pane(id).expect("scroll pane").max_scroll_pos();
    if max.y <= 10.0 {
        // 内容不足以滚动时无从测试惯性，直接跳过。
        return;
    }

    store.set_scroll_pos(id, 0.0, max.y, false);
    let before = store.scroll_pane(id).expect("scroll pane").pos().y;

    // 越界后推进若干帧，位置应当被拉回边界。
    store.set_scroll_pos(id, 0.0, -100.0, true);
    for _ in 0..180 {
        store.advance_scrolls(1.0 / 60.0);
    }

    let after = store.scroll_pane(id).expect("scroll pane").pos().y;
    assert!(
        after >= -1.0,
        "越界后应当回到合法范围，实际 {after}（此前为 {before}）"
    );
}
