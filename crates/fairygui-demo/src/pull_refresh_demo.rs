//! PullToRefreshDemo business timing using ECS entities, data and pull commands.
use crate::demos;
use bevy::prelude::*;
use fairygui_bevy::prelude::*;
use fairygui_core::object::EventKind;

#[derive(Default, Resource)]
pub(crate) struct PullDemo {
    root: Option<Entity>,
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
fn page(controllers: &mut Query<&mut GuiControllers>, target: Entity, index: i32) {
    if let Ok(mut states) = controllers.get_mut(target)
        && let Some(state) = states.get_mut("c1")
    {
        state.selected_index = index;
    }
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
    mut gui: ParamSet<(GuiQuery, Query<&mut GuiControllers>)>,
    mut releases: MessageReader<GuiPullRelease>,
    mut commands: Commands,
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
    let Some((first, second, header, footer, counts)) = ({
        let query = gui.p0();
        query
            .child_by_name(root, "list1")
            .zip(query.child_by_name(root, "list2"))
            .and_then(|(first, second)| {
                let header = *query.scroll(first)?.header()?;
                let footer = *query.scroll(second)?.footer()?;
                Some((
                    first,
                    second,
                    header,
                    footer,
                    [
                        query.list(first).map(|l| l.item_count()),
                        query.list(second).map(|l| l.item_count()),
                    ],
                ))
            })
    }) else {
        return;
    };
    if state.root != Some(root) {
        state.root = Some(root);
        state.jobs.clear();
        for (list, reverse) in [(first, true), (second, false)] {
            commands.entity(list).insert(content(1, reverse));
        }
        page(&mut gui.p1(), header.entity, 0);
        page(&mut gui.p1(), footer.entity, 0);
    }
    let mut status = gui
        .p1()
        .get(header.entity)
        .ok()
        .and_then(|states| states.get("c1"))
        .map_or(0, |state| state.selected_index);
    if status != 2 && status != 3 {
        status = i32::from(header.size.y > header.source_size.y);
        page(&mut gui.p1(), header.entity, status);
    }
    for release in events {
        let is_header = release.event.kind == EventKind::PullDownRelease && release.entity == first;
        let is_footer = release.event.kind == EventKind::PullUpRelease && release.entity == second;
        if is_header {
            if status != 1 {
                continue;
            }
            status = 2;
            page(&mut gui.p1(), header.entity, 2);
            lock(&mut commands, first, header.source_size.y, true);
        } else if is_footer {
            page(&mut gui.p1(), footer.entity, 1);
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
        let Some(count) = counts[usize::from(job.list == second)] else {
            return false;
        };
        if time.elapsed_secs_f64() < job.due {
            return true;
        }
        if !job.completion {
            commands
                .entity(job.list)
                .insert(content(count + 5, job.header));
            if job.header {
                page(&mut gui.p1(), header.entity, 3);
                lock(&mut commands, first, 35., true);
                job.completion = true;
                job.due = time.elapsed_secs_f64() + 2.;
                return true;
            }
            page(&mut gui.p1(), footer.entity, 0);
            lock(&mut commands, second, 0., false);
        } else {
            page(&mut gui.p1(), header.entity, 0);
            lock(&mut commands, first, 0., true);
        }
        false
    });
}
