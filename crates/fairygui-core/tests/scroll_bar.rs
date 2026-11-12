//! 滚动条的集成测试，使用官方示例包中的真实配置。
//!
//! 示例包里的滚动条资源地址指向包内条目，因此可以完整走通
//! 「按地址实例化 → 加入容器 → 摆放 → 随滚动更新」这条链路。

use fairygui_core::GObjectId;
use fairygui_core::object::ObjectStore;
use fairygui_core::package::{FuiPackage, LoadContext, PackageItemType};
use fairygui_core::widget::{ControlData, create_from_package};

/// 示例包中只有聊天界面配了滚动条，因此以它为夹具。
const FIXTURES: &[(&str, &[u8])] = &[("Chat", include_bytes!("fixtures/Chat.fui"))];

/// 从全部夹具里找出第一个配置了滚动条的容器。
fn any_with_bar() -> Option<(FuiPackage, ObjectStore, GObjectId)> {
    for (name, bytes) in FIXTURES {
        let Ok(package) = FuiPackage::parse(*name, bytes) else {
            continue;
        };
        if let Some((store, id)) = find_with_bar(&package) {
            return Some((package, store, id));
        }
    }
    None
}

/// 在包内所有组件里找出第一个配置了滚动条地址的容器。
fn find_with_bar(package: &FuiPackage) -> Option<(ObjectStore, GObjectId)> {
    for (index, item) in package.items().iter().enumerate() {
        if item.item_type != PackageItemType::Component {
            continue;
        }

        let mut store = ObjectStore::new();
        let Ok(root) = create_from_package(&mut store, package, index, &LoadContext::none()) else {
            continue;
        };

        let mut candidates = vec![root];
        store.for_each_descendant(root, &mut |id| candidates.push(id));

        // 只认确实配了滚动条地址的容器。
        for id in candidates {
            let has_res = store.scroll_pane(id).is_some_and(|pane| {
                pane.vertical_bar_res.is_some() || pane.horizontal_bar_res.is_some()
            });
            if has_res {
                return Some((store, id));
            }
        }
    }
    None
}

/// 按包内地址创建滚动条，返回创建出的数量。
fn attach(store: &mut ObjectStore, package: &FuiPackage, container: GObjectId) -> usize {
    store.attach_scroll_bars(container, &mut |store, url| {
        let index = package.resolve_local_url(url)?;
        create_from_package(store, package, index, &LoadContext::none()).ok()
    })
}

/// 在滚动条内按名字找滑块。
fn find_grip(store: &ObjectStore, bar: GObjectId) -> Option<GObjectId> {
    let children = store.object(bar)?.children().to_vec();
    children.into_iter().find(|child| {
        store
            .object(*child)
            .is_some_and(|object| object.name() == fairygui_core::scroll::GRIP_NAME)
    })
}

#[test]
fn sample_packages_configure_scroll_bars() {
    assert!(
        any_with_bar().is_some(),
        "官方示例包中应当有容器配置了滚动条"
    );
}

#[test]
fn scroll_bars_are_created_from_url() {
    let Some((package, mut store, container)) = any_with_bar() else {
        panic!("需要一个配了滚动条的容器");
    };

    let created = attach(&mut store, &package, container);
    assert!(created > 0, "应当按地址创建出滚动条");

    let children = store
        .object(container)
        .expect("container")
        .children()
        .to_vec();
    let pane = store.scroll_pane(container).expect("pane").clone();

    let bars: Vec<GObjectId> = [pane.vt_bar, pane.hz_bar].into_iter().flatten().collect();
    assert_eq!(bars.len(), created, "登记的数量应当与创建的一致");

    for bar in bars {
        assert!(children.contains(&bar), "滚动条应当在容器的子对象里");
        assert!(
            matches!(
                store.object(bar).map(|object| object.control()),
                Some(ControlData::ScrollBar(_))
            ),
            "创建出来的应当是滚动条控件"
        );
    }
}

#[test]
fn scroll_bars_are_placed_at_container_edges() {
    let Some((package, mut store, container)) = any_with_bar() else {
        panic!("需要一个配了滚动条的容器");
    };

    attach(&mut store, &package, container);
    store.layout_scroll_bars(container);

    let pane = store.scroll_pane(container).expect("pane").clone();
    let owner_size = store.object(container).expect("container").size();

    // 纵向滚动条贴右，横向滚动条贴底。
    if let Some(vt) = pane.vt_bar {
        let bar = store.object(vt).expect("vt bar");
        assert!(
            (bar.x() + bar.width() - owner_size.width).abs() < 1.0,
            "纵向滚动条应当贴在容器右侧：x={} 宽={} 容器宽={}",
            bar.x(),
            bar.width(),
            owner_size.width
        );
        assert!(bar.height() > 0.0, "滚动条应当有高度");
    }

    if let Some(hz) = pane.hz_bar {
        let bar = store.object(hz).expect("hz bar");
        assert!(
            (bar.y() + bar.height() - owner_size.height).abs() < 1.0,
            "横向滚动条应当贴在容器底部"
        );
    }

    // 非浮起时视口要扣除滚动条。
    assert!(pane.view_size().width <= owner_size.width + 1.0);
    assert!(pane.view_size().height <= owner_size.height + 1.0);
}

#[test]
fn grip_follows_scroll_position() {
    let Some((package, mut store, container)) = any_with_bar() else {
        panic!("需要一个配了滚动条的容器");
    };

    attach(&mut store, &package, container);
    store.layout_scroll_bars(container);

    // 造出可滚动的范围，否则滑块会因为无处可拖而隐藏。
    store.update_scroll_content_size(container, fairygui_core::geom::Size::new(2000.0, 2000.0));

    let pane = store.scroll_pane(container).expect("pane").clone();
    let Some(bar) = pane.vt_bar.or(pane.hz_bar) else {
        panic!("应当至少有一条滚动条");
    };
    let vertical = pane.vt_bar.is_some();

    let grip = find_grip(&store, bar).expect("滚动条内应当有 grip");

    store.sync_scroll_bars(container);
    assert!(
        store.object(grip).expect("grip").visible(),
        "内容超出视口时滑块应当可见"
    );

    let before = if vertical {
        store.object(grip).expect("grip").y()
    } else {
        store.object(grip).expect("grip").x()
    };

    // 滚到一半，滑块应当跟着走。
    store.set_scroll_pos(container, 1000.0, 1000.0, false);
    store.sync_scroll_bars(container);

    let after = if vertical {
        store.object(grip).expect("grip").y()
    } else {
        store.object(grip).expect("grip").x()
    };

    assert!(
        after > before,
        "滚动后滑块应当朝终点移动（{before} -> {after}）"
    );
}

#[test]
fn grip_hides_when_content_fits() {
    let Some((package, mut store, container)) = any_with_bar() else {
        panic!("需要一个配了滚动条的容器");
    };

    attach(&mut store, &package, container);
    store.layout_scroll_bars(container);

    // 内容不超出视口，滑块无处可拖，应当隐藏。
    store.update_scroll_content_size(container, fairygui_core::geom::Size::new(1.0, 1.0));
    store.sync_scroll_bars(container);

    let pane = store.scroll_pane(container).expect("pane").clone();
    for bar in [pane.vt_bar, pane.hz_bar].into_iter().flatten() {
        if let Some(grip) = find_grip(&store, bar) {
            assert!(
                !store.object(grip).expect("grip").visible(),
                "内容不超出时滑块应当隐藏"
            );
        }
    }
}

#[test]
fn click_scrolls_the_container_by_one_step() {
    let Some((package, mut store, container)) = any_with_bar() else {
        panic!("需要一个配了滚动条的容器");
    };

    attach(&mut store, &package, container);
    store.layout_scroll_bars(container);
    store.update_scroll_content_size(container, fairygui_core::geom::Size::new(2000.0, 2000.0));

    let pane = store.scroll_pane(container).expect("pane").clone();
    let vertical = pane.vt_bar.is_some();

    assert!(
        store.scroll_container_by(container, !vertical, 1.0),
        "应当能按步长滚动"
    );
    assert_eq!(store.scroll_pane(container).expect("pane").pos().y, 25.0);

    assert!(
        store.scroll_container_by(container, !vertical, -1.0),
        "应当能滚回起点"
    );
    assert_eq!(store.scroll_pane(container).expect("pane").pos().y, 0.0);
}

#[test]
fn grip_track_appears_only_when_content_overflows() {
    let Some((package, mut store, container)) = any_with_bar() else {
        panic!("需要一个配了滚动条的容器");
    };

    attach(&mut store, &package, container);
    store.layout_scroll_bars(container);

    let pane = store.scroll_pane(container).expect("pane").clone();
    let Some(bar) = pane.vt_bar.or(pane.hz_bar) else {
        panic!("应当至少有一条滚动条");
    };

    // 内容刚好放得下时滑块占满轨道，行程为零。
    store.update_scroll_content_size(container, fairygui_core::geom::Size::new(1.0, 1.0));
    store.sync_scroll_bars(container);
    assert_eq!(
        store.scroll_grip_track(bar),
        None,
        "滑块占满轨道时没有可拖动的行程"
    );

    // 内容远大于视口时滑块变短，行程随之出现。
    store.update_scroll_content_size(container, fairygui_core::geom::Size::new(5000.0, 5000.0));
    store.sync_scroll_bars(container);
    assert!(
        store
            .scroll_grip_track(bar)
            .is_some_and(|track| track > 0.0),
        "内容超出视口后应当出现可拖动行程"
    );
}

#[test]
fn whole_bar_follows_scrollability() {
    let Some((package, mut store, container)) = any_with_bar() else {
        panic!("需要一个配了滚动条的容器");
    };

    attach(&mut store, &package, container);
    store.layout_scroll_bars(container);
    store.set_scroll_bar_display(
        container,
        fairygui_core::scroll::ScrollBarDisplayType::Visible,
    );

    let pane = store.scroll_pane(container).expect("pane").clone();
    let bars: Vec<GObjectId> = [pane.vt_bar, pane.hz_bar].into_iter().flatten().collect();
    assert!(!bars.is_empty(), "应当至少有一条滚动条");

    // Native content-fit hiding is gated by the exported display-in-demand bit.
    store.update_scroll_content_size(container, fairygui_core::geom::Size::new(1.0, 1.0));
    store.set_scroll_pos(container, 10.0, 10.0, false);
    store.sync_scroll_bars(container);
    for bar in &bars {
        assert_eq!(
            store.object(*bar).expect("bar").visible(),
            !pane.display_in_demand
        );
    }

    // 内容远大于视口：滚动一次后重新显示。
    store.update_scroll_content_size(container, fairygui_core::geom::Size::new(5000.0, 5000.0));
    store.set_scroll_pos(container, 100.0, 100.0, false);
    store.sync_scroll_bars(container);
    for bar in &bars {
        assert!(
            store.object(*bar).expect("bar").visible(),
            "内容超出时滚动条应当显示"
        );
    }
}

#[test]
fn bar_min_size_only_gates_when_arrows_exist() {
    let Some((package, mut store, container)) = any_with_bar() else {
        panic!("需要一个配了滚动条的容器");
    };

    attach(&mut store, &package, container);
    store.set_scroll_bar_display(
        container,
        fairygui_core::scroll::ScrollBarDisplayType::Visible,
    );

    let pane = store.scroll_pane(container).expect("pane").clone();
    let Some(bar) = pane.vt_bar.or(pane.hz_bar) else {
        panic!("应当至少有一条滚动条");
    };

    // 示例包的滚动条没有两端箭头，最小尺寸为 0，因此「视口太小」这条不会生效。
    assert_eq!(
        store.scroll_bar_min_size(bar),
        0.0,
        "没有箭头时最小尺寸为 0"
    );

    // 即便容器被压到极小，只要内容超出就仍然显示——此时起作用的只有可滚动性。
    // 滚动一次让按需显示的滚动条显形。
    store.set_size(container, 0.5, 0.5);
    store.layout_scroll_bars(container);
    store.update_scroll_content_size(container, fairygui_core::geom::Size::new(5000.0, 5000.0));
    store.set_scroll_pos(container, 1.0, 1.0, false);
    store.sync_scroll_bars(container);

    assert!(
        store.object(bar).expect("bar").visible(),
        "没有箭头时不应因为视口小而隐藏"
    );
}

#[test]
fn on_demand_bars_hide_after_scrolling_stops() {
    let Some((package, mut store, container)) = any_with_bar() else {
        panic!("需要一个配了滚动条的容器");
    };

    attach(&mut store, &package, container);
    store.layout_scroll_bars(container);

    // 示例包配置的正是按需显示，这里先确认这一点。
    assert_eq!(
        store.scroll_pane(container).expect("pane").bar_display,
        fairygui_core::scroll::ScrollBarDisplayType::Auto,
        "示例包应当是按需显示"
    );

    store.update_scroll_content_size(container, fairygui_core::geom::Size::new(5000.0, 5000.0));
    store.sync_scroll_bars(container);

    let pane = store.scroll_pane(container).expect("pane").clone();
    let Some(bar) = pane.vt_bar.or(pane.hz_bar) else {
        panic!("应当至少有一条滚动条");
    };

    // 按需显示模式下，还没滚动过时是隐藏的。
    assert!(
        !store.object(bar).expect("bar").visible(),
        "尚未滚动时按需显示的滚动条应当隐藏"
    );

    // Immediate native setters leave a hidden auto bar hidden; active dragging wakes it.
    store.set_scroll_pos(container, 100.0, 100.0, false);
    store.sync_scroll_bars(container);
    assert!(!store.object(bar).expect("bar").visible());
    store.begin_scroll_drag(container);
    store.end_scroll_drag(container);
    assert!(store.object(bar).expect("bar").visible(), "滚动时应当可见");

    // 停止滚动后推进到淡出一半的位置，此时各部件应当已经变透明。
    for _ in 0..45 {
        store.advance_scrolls(1.0 / 60.0);
    }
    store.sync_scroll_bars(container);

    let faded = store.object(bar).expect("bar").alpha();
    assert!(faded < 1.0, "停止滚动后应当开始淡出，实际 {faded}");

    // 部件的自身透明度不变，但有效透明度会跟着容器一起降下来——
    // 透明度是沿子树累积的，容器变淡就等于整条变淡。
    let parts = store.object(bar).expect("bar").children().to_vec();
    assert!(!parts.is_empty(), "滚动条应当有子部件");
    for part in parts {
        assert!(
            store.effective_alpha(part) < 1.0,
            "各部件的有效透明度要跟着容器一起降"
        );
    }

    // 再推进足够长时间：整条完全淡出并隐藏。
    for _ in 0..90 {
        store.advance_scrolls(1.0 / 60.0);
    }
    store.sync_scroll_bars(container);

    assert_eq!(
        store.object(bar).expect("bar").alpha(),
        1.0,
        "native completion restores alpha"
    );
    assert!(
        !store.object(bar).expect("bar").visible(),
        "停止滚动一段时间后应当淡出"
    );
}

#[test]
fn bar_display_hidden_creates_nothing() {
    let Some((package, mut store, container)) = any_with_bar() else {
        panic!("需要一个配了滚动条的容器");
    };

    // 把显示方式改成「不显示」，此时根本不应创建滚动条。
    assert!(store.set_scroll_bar_display(
        container,
        fairygui_core::scroll::ScrollBarDisplayType::Hidden
    ));

    assert_eq!(
        attach(&mut store, &package, container),
        0,
        "配置为不显示时不应创建滚动条"
    );
}

#[test]
fn dragging_grip_moves_the_container() {
    let Some((package, mut store, container)) = any_with_bar() else {
        panic!("需要一个配了滚动条的容器");
    };

    attach(&mut store, &package, container);
    store.layout_scroll_bars(container);
    store.update_scroll_content_size(container, fairygui_core::geom::Size::new(2000.0, 2000.0));

    let vertical = store.scroll_pane(container).expect("pane").vt_bar.is_some();
    let perc = |store: &ObjectStore| {
        let pane = store.scroll_pane(container).expect("pane");
        if vertical {
            pane.perc_y()
        } else {
            pane.perc_x()
        }
    };

    assert_eq!(perc(&store), 0.0, "初始在起点");

    // 把滑块推到中间，滚动位置应当落在行程的一半。
    assert!(store.set_scroll_perc(container, vertical, 0.5));
    assert!((perc(&store) - 0.5).abs() < 1e-6);

    // 推到终点。
    assert!(store.set_scroll_perc(container, vertical, 1.0));
    assert_eq!(perc(&store), 1.0);

    // 超出范围的比例会被夹住，不会滚到内容之外。
    assert!(!store.set_scroll_perc(container, vertical, 3.0));
    assert_eq!(perc(&store), 1.0);

    let pane = store.scroll_pane(container).expect("pane");
    let (pos, max) = (pane.pos(), pane.max_scroll_pos());
    assert_eq!(
        if vertical { pos.y } else { pos.x },
        if vertical { max.y } else { max.x },
        "拖到终点时应当停在最大滚动位置"
    );
}

#[test]
fn bar_reports_its_axis() {
    let Some((package, mut store, container)) = any_with_bar() else {
        panic!("需要一个配了滚动条的容器");
    };

    attach(&mut store, &package, container);
    let pane = store.scroll_pane(container).expect("pane").clone();

    if let Some(vt) = pane.vt_bar {
        assert_eq!(store.scroll_bar_axis(vt), Some(true), "纵向滚动条");
        assert_eq!(store.scroll_container_of_bar(vt), Some(container));
    }
    if let Some(hz) = pane.hz_bar {
        assert_eq!(store.scroll_bar_axis(hz), Some(false), "横向滚动条");
        assert_eq!(store.scroll_container_of_bar(hz), Some(container));
    }
}
