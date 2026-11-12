//! Original five-icon loop carousel and its scroll-time scale/label behavior.
use super::*;

pub fn initialize(runtime: &mut GuiRuntime, root: GObjectId) {
    let list = runtime
        .store()
        .child_by_name(root, "list")
        .expect("LoopList list");
    let icons: Vec<_> = (1..=5)
        .map(|i| {
            runtime
                .registry()
                .item_url("LoopList", &format!("n{i}"))
                .expect("LoopList icon")
        })
        .collect();
    runtime
        .enable_loop_virtual_list(list, move |store, index, row| {
            store.set_pivot(row, 0.5, 0.5, false);
            store.apply_icon_to(row, &icons[index]);
        })
        .expect("LoopList template");
    runtime
        .set_virtual_item_count(list, 5)
        .expect("LoopList renderer");
}

pub fn update(mut runtime: NonSendMut<GuiRuntime>, controller: Res<demos::DemoController>) {
    if controller.package.as_deref() != Some("LoopList") {
        return;
    }
    let Some(root) = controller.root else {
        return;
    };
    let Some(list) = runtime.store().child_by_name(root, "list") else {
        return;
    };
    runtime
        .refresh_virtual_list(list)
        .expect("LoopList scroll renderer");
    let pane = runtime.store().scroll_pane(list).unwrap();
    let mid = pane.pos().x + pane.view_size().width / 2.;
    for row in runtime.store().list_items(list).unwrap() {
        let o = runtime.store().object(row).unwrap();
        let distance = (mid - o.x() - o.width() / 2.).abs();
        let scale = if distance > o.width() {
            1.
        } else {
            1. + (1. - distance / o.width()) * 0.24
        };
        runtime.store_mut().set_scale(row, scale, scale);
    }
    let count = runtime.store().list_num_items(list);
    if count > 0
        && let Some(first) = runtime.store().list_first_item_in_view(list)
        && let Some(label) = runtime.store().child_by_name(root, "n3")
    {
        runtime
            .store_mut()
            .set_object_text(label, &((first + 1) % count).to_string());
    }
}
