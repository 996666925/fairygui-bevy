//! PullToRefreshDemo business timing using ECS entities, data and pull commands.
use crate::demos;
use bevy::prelude::*;
use fairygui_bevy::prelude::*;
use fairygui_core::object::EventKind;

#[derive(Default, Resource)]
pub(crate) struct PullDemo {
    root: Option<Entity>,
    lists: [Option<Entity>; 2],
    jobs: Vec<Job>,
}
struct Job {
    list: Entity,
    header: bool,
    due: f64,
    completion: bool,
}

fn content(count: usize, reverse: bool) -> GuiListContent {
    GuiListContent::new(count, move |index, row| {
        let index = if reverse { count - index - 1 } else { index };
        row.set_text(&format!("Item {index}"));
    })
}
fn page(gui_edit: &mut GuiMut, target: Entity, index: i32) {
    gui_edit.edit(target, |object: &mut GuiObjectMut| {
        object.set_controller_page("c1", index)
    });
}
fn lock(commands: &mut Commands, target: Entity, size: f32, header: bool) {
    commands.write_message(if header {
        GuiPullCommand::LockHeader { target, size }
    } else {
        GuiPullCommand::LockFooter { target, size }
    });
}

pub(crate) fn update(
    mut state: ResMut<PullDemo>,
    controller: Res<demos::DemoController>,
    gui: GuiQuery,
    mut releases: MessageReader<GuiPullRelease>,
    mut commands: Commands,
    mut gui_edit: GuiMut,
    time: Res<Time>,
) {
    let events: Vec<_> = releases.read().cloned().collect();
    let root = controller.root_owner;
    if controller.package.as_deref() != Some("PullToRefresh") || root.is_none() {
        state.root = None;
        state.jobs.clear();
        return;
    }
    let root = root.unwrap();
    if state.root != Some(root) {
        let Some((first, second)) = gui
            .child_by_name(root, "list1")
            .zip(gui.child_by_name(root, "list2"))
        else {
            return;
        };
        let Some((header, footer)) = gui
            .scroll(first)
            .and_then(|scroll| scroll.header())
            .zip(gui.scroll(second).and_then(|scroll| scroll.footer()))
        else {
            return;
        };
        state.root = Some(root);
        state.lists = [Some(first), Some(second)];
        state.jobs.clear();
        for (list, reverse) in [(first, true), (second, false)] {
            commands.entity(list).insert(content(1, reverse));
        }
        page(&mut gui_edit, header.entity, 0);
        page(&mut gui_edit, footer.entity, 0);
    }
    let [Some(first), Some(second)] = state.lists else {
        return;
    };
    let Some((header, footer)) = gui
        .scroll(first)
        .and_then(|scroll| scroll.header())
        .zip(gui.scroll(second).and_then(|scroll| scroll.footer()))
    else {
        return;
    };
    let mut status = gui
        .controller(header.entity, "c1")
        .map_or(0, |controller| controller.selected_index());
    if status != 2 && status != 3 {
        status = i32::from(header.size.y > header.source_size.y);
        page(&mut gui_edit, header.entity, status);
    }
    for release in events {
        let is_header = release.event.kind == EventKind::PullDownRelease && release.entity == first;
        let is_footer = release.event.kind == EventKind::PullUpRelease && release.entity == second;
        if is_header {
            if status != 1 {
                continue;
            }
            status = 2;
            page(&mut gui_edit, header.entity, 2);
            lock(&mut commands, first, header.source_size.y, true);
        } else if is_footer {
            page(&mut gui_edit, footer.entity, 1);
            lock(&mut commands, second, footer.source_size.y, false);
        } else {
            continue;
        }
        state.jobs.push(Job {
            list: release.entity,
            header: is_header,
            due: time.elapsed_secs_f64() + 2.,
            completion: false,
        });
    }
    state.jobs.retain_mut(|job| {
        let Some(list) = gui.list(job.list) else {
            return false;
        };
        if time.elapsed_secs_f64() < job.due {
            return true;
        }
        if !job.completion {
            commands
                .entity(job.list)
                .insert(content(list.item_count() + 5, job.header));
            if job.header {
                page(&mut gui_edit, header.entity, 3);
                lock(&mut commands, first, 35., true);
                job.completion = true;
                job.due = time.elapsed_secs_f64() + 2.;
                return true;
            }
            page(&mut gui_edit, footer.entity, 0);
            lock(&mut commands, second, 0., false);
        } else {
            page(&mut gui_edit, header.entity, 0);
            lock(&mut commands, first, 0., true);
        }
        false
    });
}
