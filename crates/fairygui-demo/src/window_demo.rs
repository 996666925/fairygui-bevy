//! Basics window buttons own retained native windows through ECS.
use bevy::prelude::*;
use fairygui_bevy::prelude::*;

#[derive(Component)]
pub(crate) struct Bound;
#[derive(Component)]
struct ShowWindow(Entity);

pub fn bind(
    mut commands: Commands,
    pages: Query<(Entity, &FairyGuiRoot), Without<Bound>>,
    gui: GuiQuery,
) {
    for (page, source) in &pages {
        if source.item_name != "Demo_Window" || gui.view(page).is_none() {
            continue;
        }
        for (button_name, template, modal) in [("n0", "WindowA", false), ("n1", "WindowB", true)] {
            let Some(button) = gui.child_by_name(page, button_name) else {
                continue;
            };
            let mut window = FairyGuiWindow::new(source.asset.clone(), template);
            window.visible = false;
            window.modal = modal;
            let owner = commands.spawn((window, ChildOf(page))).id();
            commands
                .entity(button)
                .insert(ShowWindow(owner))
                .observe(show);
        }
        commands.entity(page).insert(Bound);
    }
}
fn show(event: On<GuiEvent>, buttons: Query<&ShowWindow>, mut commands: Commands) {
    if event.event().event.kind != fairygui_bevy::core::object::EventKind::Click {
        return;
    }
    if let Ok(button) = buttons.get(event.entity) {
        commands.write_message(GuiWindowCommand::new(button.0, GuiWindowAction::Show));
        commands.write_message(GuiWindowCommand::new(
            button.0,
            GuiWindowAction::BringToFront,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::SystemState;
    #[test]
    fn exported_window_buttons_show_retained_children_and_page_disposal_owns_them() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            FairyGuiPlugin,
        ))
        .init_resource::<ButtonInput<MouseButton>>()
        .add_message::<bevy::input::mouse::MouseWheel>()
        .add_systems(Update, bind.after(GuiSystems::Sync));
        let package = fairygui_bevy::core::package::FuiPackage::parse(
            "Basics",
            include_bytes!("../assets/ui/Basics.fui"),
        )
        .unwrap();
        let asset = app
            .world_mut()
            .resource_mut::<Assets<FuiAsset>>()
            .add(FuiAsset {
                package,
                res_key: "ui/Basics".into(),
            });
        let page = app
            .world_mut()
            .spawn(FairyGuiRoot::new(asset, "Demo_Window"))
            .id();
        app.update();
        app.update();
        app.update();
        let mut gui = SystemState::<GuiQuery>::new(app.world_mut());
        let mut owners = Vec::new();
        let mut views = Vec::new();
        for (name, modal) in [("n0", false), ("n1", true)] {
            let button = gui
                .get(app.world())
                .unwrap()
                .child_by_name(page, name)
                .unwrap();
            let owner = app
                .world()
                .get::<ShowWindow>(button)
                .expect("button bound to window")
                .0;
            let view = app.world().get::<GuiWindowState>(owner).unwrap().view();
            assert!(
                !app.world()
                    .get::<GuiWindowState>(owner)
                    .unwrap()
                    .is_showing()
            );
            let object = app.world().get::<GObjectRef>(button).unwrap().0;
            app.world_mut().trigger(GuiEvent {
                entity: button,
                event: fairygui_bevy::core::object::UiEvent::new(
                    fairygui_bevy::core::object::EventKind::Click,
                    object,
                ),
            });
            app.update();
            let state = app.world().get::<GuiWindowState>(owner).unwrap();
            assert!(state.is_showing());
            assert_eq!(state.is_modal(), modal);
            assert_eq!(state.view(), view);
            owners.push(owner);
            views.push(view);
        }
        app.world_mut().despawn(page);
        app.update();
        assert!(
            owners
                .iter()
                .chain(&views)
                .all(|&entity| app.world().get_entity(entity).is_err())
        );
        assert_eq!(app.world().non_send::<GuiRuntime>().window_count(), 0);
    }
}
