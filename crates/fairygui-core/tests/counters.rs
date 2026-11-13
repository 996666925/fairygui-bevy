//! 进度条与滑动条的集成测试。

use fairygui_core::GObjectId;
use fairygui_core::object::ObjectStore;
use fairygui_core::package::{FuiPackage, LoadContext, PackageItemType};
use fairygui_core::widget::{ControlData, create_from_package};

const BASICS: &[u8] = include_bytes!("fixtures/Basics.fui");

/// 在包内所有组件里找出第一个满足条件的控件。
fn find_widget(
    package: &FuiPackage,
    pick: fn(&ControlData) -> bool,
) -> Option<(ObjectStore, GObjectId)> {
    for (index, item) in package.items().iter().enumerate() {
        if item.item_type != PackageItemType::Component {
            continue;
        }

        let mut store = ObjectStore::new();
        let Ok(root) = create_from_package(&mut store, package, index, &LoadContext::none()) else {
            continue;
        };

        let mut found = None;
        if store
            .object(root)
            .is_some_and(|object| pick(object.control()))
        {
            found = Some(root);
        }

        let mut visit = |id: GObjectId| {
            if found.is_none()
                && let Some(object) = store.object(id)
                && pick(object.control())
            {
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
fn progress_value_is_clamped_and_reports_changes() {
    let package = FuiPackage::parse("Basics", BASICS).expect("parse basics package");
    let Some((mut store, id)) = find_widget(&package, |control| {
        matches!(control, ControlData::ProgressBar(_))
    }) else {
        return;
    };

    let Some(ControlData::ProgressBar(data)) = store.object(id).map(|o| o.control()) else {
        return;
    };
    let (min, max) = (data.min, data.max);

    // 越界值应当被夹到边界上。
    assert!(store.set_progress(id, max + 10_000));
    assert_eq!(store.progress_value(id), Some(max));

    assert!(store.set_progress(id, min - 10_000));
    assert_eq!(store.progress_value(id), Some(min));

    // 设成同一个值不应再报告变化。
    assert!(!store.set_progress(id, min));

    // 中间值应当生效。
    let middle = (min + max) / 2;
    if middle != min {
        assert!(store.set_progress(id, middle));
        assert_eq!(store.progress_value(id), Some(middle));
    }
}

#[test]
fn progress_bar_scales_its_bar_child() {
    let package = FuiPackage::parse("Basics", BASICS).expect("parse basics package");
    let Some((mut store, id)) = find_widget(&package, |control| {
        matches!(control, ControlData::ProgressBar(_))
    }) else {
        return;
    };

    // 只在该进度条确实有名为 bar 的子对象时才验证缩放。
    let Some(bar) = store.child_by_name(id, "bar") else {
        return;
    };

    let Some(ControlData::ProgressBar(data)) = store.object(id).map(|o| o.control()) else {
        return;
    };
    let (min, max) = (data.min, data.max);
    if max <= min {
        return;
    }

    // 归零时进度条应当收缩到 0，满值时回到原始尺寸。
    store.set_progress(id, min);
    let at_min = store.object(bar).map(|o| o.size()).unwrap_or_default();

    store.set_progress(id, max);
    let at_max = store.object(bar).map(|o| o.size()).unwrap_or_default();

    assert!(
        at_max.width >= at_min.width && at_max.height >= at_min.height,
        "满值时的尺寸不应小于归零时（{at_min:?} → {at_max:?}）"
    );
    assert!(
        at_min.width <= 0.5 || at_min.height <= 0.5,
        "归零时进度方向应当收缩到接近 0，实际 {at_min:?}"
    );
}

#[test]
fn slider_value_moves_its_grip() {
    let package = FuiPackage::parse("Basics", BASICS).expect("parse basics package");
    let Some((mut store, id)) = find_widget(&package, |control| {
        matches!(control, ControlData::Slider(_))
    }) else {
        return;
    };

    let Some(ControlData::Slider(data)) = store.object(id).map(|o| o.control()) else {
        return;
    };
    let (min, max) = (data.min, data.max);
    if max <= min {
        return;
    }

    assert!(store.set_slider_value(id, min));
    assert_eq!(store.slider_value(id), Some(min));

    assert!(store.set_slider_value(id, max));
    assert_eq!(store.slider_value(id), Some(max));

    // Native programmatic assignment preserves the requested value.
    assert!(store.set_slider_value(id, max + 100.));
    assert_eq!(store.slider_value(id), Some(max + 100.));
}
