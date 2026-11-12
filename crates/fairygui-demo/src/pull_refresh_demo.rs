//! PullToRefreshDemo business timing, using the exported Header/Footer controls.
use super::*;
use fairygui_core::{
    object::EventKind,
    package::LoadContext,
    widget::{ControlData, create_from_package},
};

#[derive(Default, Resource)]
pub(crate) struct PullDemo {
    root: Option<GObjectId>,
    lists: [Option<GObjectId>; 2],
    jobs: Vec<Job>,
}
struct Job {
    list: GObjectId,
    header: bool,
    due: f64,
    completion: bool,
}

fn update_titles(runtime: &mut GuiRuntime, list: GObjectId, reverse: bool) {
    let rows = runtime.store().list_items(list).unwrap_or_default();
    for (i, &row) in rows.iter().enumerate() {
        let index = if reverse { rows.len() - i - 1 } else { i };
        runtime
            .store_mut()
            .set_object_text(row, &format!("Item {index}"));
    }
    runtime.store_mut().layout_list(list);
    runtime.store_mut().layout_scroll_bars(list);
}

fn add_rows(runtime: &mut GuiRuntime, list: GObjectId, reverse: bool) {
    let url = runtime
        .store()
        .object(list)
        .and_then(|o| match o.control() {
            ControlData::List(d) => d.default_item.clone(),
            _ => None,
        });
    let Some(url) = url else {
        return;
    };
    let Some((package, item)) = runtime.resolve_url(&url).map(|(p, i)| (p.clone(), i)) else {
        return;
    };
    for _ in 0..5 {
        if let Ok(row) =
            create_from_package(runtime.store_mut(), &package, item, &LoadContext::none())
        {
            runtime.store_mut().set_parent(row, Some(list));
            runtime.queue_subtree(row);
        }
    }
    update_titles(runtime, list, reverse);
}

pub(crate) fn update(
    mut state: ResMut<PullDemo>,
    controller: Res<demos::DemoController>,
    mut runtime: NonSendMut<GuiRuntime>,
    mut releases: MessageReader<GuiPullRelease>,
    time: Res<Time>,
) {
    let events: Vec<_> = releases.read().cloned().collect();
    if controller.package.as_deref() != Some("PullToRefresh") || controller.root.is_none() {
        state.root = None;
        state.jobs.clear();
        return;
    }
    let root = controller.root.unwrap();
    if state.root != Some(root) {
        let first = runtime.store().child_by_name(root, "list1");
        let second = runtime.store().child_by_name(root, "list2");
        let Some((first, second)) = first.zip(second) else {
            return;
        };
        let Some((header, footer)) = runtime
            .store()
            .scroll_header(first)
            .zip(runtime.store().scroll_footer(second))
        else {
            return;
        };
        state.root = Some(root);
        state.lists = [Some(first), Some(second)];
        state.jobs.clear();
        for (list, reverse) in [(first, true), (second, false)] {
            for row in runtime
                .store()
                .list_items(list)
                .unwrap_or_default()
                .into_iter()
                .skip(1)
            {
                runtime.store_mut().remove(row);
            }
            update_titles(&mut runtime, list, reverse);
        }
        runtime
            .store_mut()
            .set_controller_page_index(header, "c1", 0);
        runtime
            .store_mut()
            .set_controller_page_index(footer, "c1", 0);
    }
    let first = state.lists[0].unwrap();
    let second = state.lists[1].unwrap();
    let Some(header) = runtime.store().scroll_header(first) else {
        return;
    };
    let Some(footer) = runtime.store().scroll_footer(second) else {
        return;
    };
    let status = runtime
        .store()
        .object(header)
        .and_then(|o| o.control().component())
        .and_then(|c| c.controller("c1"))
        .map_or(0, |c| c.selected_index);
    if status != 2 && status != 3 {
        let object = runtime.store().object(header).unwrap();
        let ready = object.height() > object.source_size().height;
        runtime
            .store_mut()
            .set_controller_page_index(header, "c1", i32::from(ready));
    }
    for release in events {
        let is_header =
            release.event.kind == EventKind::PullDownRelease && release.event.target == first;
        let is_footer =
            release.event.kind == EventKind::PullUpRelease && release.event.target == second;
        if is_header {
            let o = runtime.store().object(header).unwrap();
            if o.control()
                .component()
                .unwrap()
                .controller("c1")
                .unwrap()
                .selected_index
                != 1
            {
                continue;
            }
            let height = o.source_size().height;
            runtime
                .store_mut()
                .set_controller_page_index(header, "c1", 2);
            runtime.store_mut().lock_scroll_header(first, height);
        } else if is_footer {
            let height = runtime.store().object(footer).unwrap().source_size().height;
            runtime
                .store_mut()
                .set_controller_page_index(footer, "c1", 1);
            runtime.store_mut().lock_scroll_footer(second, height);
        } else {
            continue;
        }
        state.jobs.push(Job {
            list: release.event.target,
            header: is_header,
            due: time.elapsed_secs_f64() + 2.,
            completion: false,
        });
    }
    state.jobs.retain_mut(|job| {
        if !runtime.store().contains(job.list) {
            return false;
        }
        if time.elapsed_secs_f64() < job.due {
            return true;
        }
        if !job.completion {
            add_rows(&mut runtime, job.list, job.header);
            if job.header {
                runtime
                    .store_mut()
                    .set_controller_page_index(header, "c1", 3);
                runtime.store_mut().lock_scroll_header(first, 35.);
                job.completion = true;
                job.due = time.elapsed_secs_f64() + 2.;
                return true;
            }
            runtime
                .store_mut()
                .set_controller_page_index(footer, "c1", 0);
            runtime.store_mut().lock_scroll_footer(second, 0.);
        } else {
            runtime
                .store_mut()
                .set_controller_page_index(header, "c1", 0);
            runtime.store_mut().lock_scroll_header(first, 0.);
        }
        false
    });
}
