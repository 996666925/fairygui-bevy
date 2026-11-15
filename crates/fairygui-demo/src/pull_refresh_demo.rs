//! PullToRefreshDemo business timing using ECS data and immediate scroll edits.
use crate::demos;
use crate::ui::pull_to_refresh::{self as ui, Footer, FooterC1Page, Header, HeaderC1Page};
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
fn page<P: GuiPage>(
    controllers: &mut Query<&mut GuiControllers>,
    controller: GuiController<P>,
    page: P,
) {
    controller
        .select(controllers, page)
        .expect("validated refresh controller");
}
fn lock(gui: &mut GuiMut, target: Entity, size: f32, header: bool) {
    if let Some(mut object) = gui.get_mut(target) {
        if header {
            object.lock_scroll_header(size);
        } else {
            object.lock_scroll_footer(size);
        }
    }
}

pub(crate) fn update(
    mut state: ResMut<PullDemo>,
    controller: Res<demos::DemoController>,
    mut gui: ParamSet<(GuiQuery, Query<&mut GuiControllers>)>,
    mut releases: MessageReader<GuiPullRelease>,
    mut commands: Commands,
    time: Res<Time>,
    mut edit: GuiMut,
) {
    let events: Vec<_> = releases.read().cloned().collect();
    let root = controller.root_owner;
    if controller.package.as_deref() != Some(ui::PACKAGE_NAME) || root.is_none() {
        state.root = None;
        state.jobs.clear();
        return;
    }
    let root = root.unwrap();
    let Some((panel, header, footer, header_ui, footer_ui, counts)) = (|| {
        let query = gui.p0();
        let panel = ui::Main::bind(&query, root).ok()?;
        let first = panel.list1.entity();
        let second = panel.list2.entity();
        let header = *query.scroll(first)?.header()?;
        let footer = *query.scroll(second)?.footer()?;
        let header_ui = Header::bind(&query, header.entity).ok()?;
        let footer_ui = Footer::bind(&query, footer.entity).ok()?;
        let counts = [
            query.list(first).map(|l| l.item_count()),
            query.list(second).map(|l| l.item_count()),
        ];
        Some((panel, header, footer, header_ui, footer_ui, counts))
    })() else {
        return;
    };
    let first = panel.list1.entity();
    let second = panel.list2.entity();
    // The materialized view changes when an owner replaces/reloads its template.
    if state.root != Some(panel.root.entity()) {
        state.root = Some(panel.root.entity());
        state.jobs.clear();
        for (list, reverse) in [(first, true), (second, false)] {
            commands.entity(list).insert(content(1, reverse));
        }
        page(&mut gui.p1(), header_ui.c_c1, HeaderC1Page::Page0);
        page(&mut gui.p1(), footer_ui.c_c1, FooterC1Page::Page0);
    }
    let mut status = header_ui
        .c_c1
        .selected(&gui.p0())
        .unwrap_or(HeaderC1Page::Page0);
    if !matches!(status, HeaderC1Page::Page2 | HeaderC1Page::Page3) {
        status = if header.size.y > header.source_size.y {
            HeaderC1Page::Page1
        } else {
            HeaderC1Page::Page0
        };
        page(&mut gui.p1(), header_ui.c_c1, status);
    }
    for release in events {
        let is_header = release.event.kind == EventKind::PullDownRelease && release.entity == first;
        let is_footer = release.event.kind == EventKind::PullUpRelease && release.entity == second;
        if is_header {
            if status != HeaderC1Page::Page1 {
                continue;
            }
            status = HeaderC1Page::Page2;
            page(&mut gui.p1(), header_ui.c_c1, status);
            lock(&mut edit, first, header.source_size.y, true);
        } else if is_footer {
            page(&mut gui.p1(), footer_ui.c_c1, FooterC1Page::Page1);
            lock(&mut edit, second, footer.source_size.y, false);
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
                page(&mut gui.p1(), header_ui.c_c1, HeaderC1Page::Page3);
                lock(&mut edit, first, 35., true);
                job.completion = true;
                job.due = time.elapsed_secs_f64() + 2.;
                return true;
            }
            page(&mut gui.p1(), footer_ui.c_c1, FooterC1Page::Page0);
            lock(&mut edit, second, 0., false);
        } else {
            page(&mut gui.p1(), header_ui.c_c1, HeaderC1Page::Page0);
            lock(&mut edit, first, 0., true);
        }
        false
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::SystemState;

    #[test]
    fn replacing_the_view_of_the_same_owner_resets_rows_and_cancels_old_refresh_jobs() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            FairyGuiPlugin,
        ))
        .init_resource::<ButtonInput<MouseButton>>()
        .add_message::<bevy::input::mouse::MouseWheel>()
        .init_resource::<PullDemo>()
        .init_resource::<demos::DemoController>()
        .add_systems(Update, update.in_set(GuiSystems::Respond));
        let package = fairygui_core::FuiPackage::parse(
            ui::PACKAGE_NAME,
            include_bytes!("../assets/ui/PullToRefresh.fui"),
        )
        .unwrap();
        let asset = app
            .world_mut()
            .resource_mut::<Assets<FuiAsset>>()
            .add(FuiAsset {
                package,
                res_key: "ui/PullToRefresh".into(),
            });
        let owner = app
            .world_mut()
            .spawn(FairyGuiRoot::new(asset, ui::Main::ITEM_NAME))
            .id();
        {
            let mut controller = app.world_mut().resource_mut::<demos::DemoController>();
            controller.root_owner = Some(owner);
            controller.package = Some(ui::PACKAGE_NAME.into());
        }
        for _ in 0..3 {
            app.update();
        }
        let mut query = SystemState::<GuiQuery>::new(app.world_mut());
        let first = ui::Main::bind(&query.get(app.world()).unwrap(), owner).unwrap();
        let old_list = first.list1.entity();
        assert_eq!(
            query
                .get(app.world())
                .unwrap()
                .list(old_list)
                .unwrap()
                .item_count(),
            1
        );
        app.world_mut().resource_mut::<PullDemo>().jobs.push(Job {
            list: old_list,
            header: true,
            due: 0.,
            completion: false,
        });

        // Keep the owner but replace its native tree. The outstanding job now
        // points to a disposed list and must never edit the replacement tree.
        app.world_mut()
            .get_mut::<FairyGuiRoot>(owner)
            .unwrap()
            .item_name = Header::ITEM_NAME.into();
        app.update();
        app.world_mut()
            .get_mut::<FairyGuiRoot>(owner)
            .unwrap()
            .item_name = ui::Main::ITEM_NAME.into();
        for _ in 0..3 {
            app.update();
        }
        let gui = query.get(app.world()).unwrap();
        let second = ui::Main::bind(&gui, owner).unwrap();
        assert_ne!(first.root.entity(), second.root.entity());
        assert!(app.world().get_entity(old_list).is_err());
        assert_eq!(gui.list(second.list1.entity()).unwrap().item_count(), 1);
        assert_eq!(gui.list(second.list2.entity()).unwrap().item_count(), 1);
        let state = app.world().resource::<PullDemo>();
        assert_eq!(state.root, Some(second.root.entity()));
        assert!(state.jobs.is_empty());
    }
}
