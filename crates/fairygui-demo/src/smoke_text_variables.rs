//! Template source, native defaults and retained display updates.
use super::*;

#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    step: u8,
    owner: Option<Entity>,
    plain_width: f32,
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<State>().add_systems(PreUpdate, drive);
}

#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    demo: Res<Demo>,
    gui: GuiQuery,
    mut ui: GuiMut,
    mut packages: ResMut<Assets<fairygui_bevy::FuiAsset>>,
    text: Query<&Text>,
    spans: Query<&TextSpan>,
    children: Query<&Children>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    assert!(
        state.frame < 700,
        "template smoke timeout at {}",
        state.step
    );
    let elapsed = state.frame - state.since;
    let mut next = false;
    match state.step {
        0 if state.frame >= 30 => {
            let Some(menu) = demo.menu_owner.filter(|&owner| gui.view(owner).is_some()) else {
                return;
            };
            ui.get_mut(menu).unwrap().set_visible(false);
            let asset = packages.add(fairygui_bevy::FuiAsset {
                package: fairygui_bevy::core::FuiPackage::parse(
                    "TextTemplate",
                    include_bytes!("../../fairygui-core/tests/fixtures/TextTemplate.fui"),
                )
                .unwrap(),
                res_key: "ui/TextTemplate".into(),
            });
            state.owner = Some(
                commands
                    .spawn(FairyGuiRoot::new(asset, "Main").with_position(Vec2::new(100., 100.)))
                    .id(),
            );
            next = true;
        }
        1 if elapsed >= 12 && gui.view(state.owner.unwrap()).is_some() => {
            let owner = state.owner.unwrap();
            let plain = gui.child_by_name(owner, "plain").unwrap();
            let input = gui.child_by_name(owner, "input").unwrap();
            let rich = gui.child_by_name(owner, "rich").unwrap();
            assert_eq!(text.get(plain).unwrap().0, "Hello Guest! ");
            assert_eq!(text.get(input).unwrap().0, "Guest");
            assert_eq!(span_text(rich, &spans, &children), "Guest / none");
            assert_eq!(gui.text(plain), Some("Hello {name=Guest}! {missing}"));
            state.plain_width = gui.transform(plain).unwrap().size().x;
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-text-vars-defaults.png"));
            next = true;
        }
        2 if elapsed >= 8 => {
            let owner = state.owner.unwrap();
            for name in ["plain", "rich", "input", "nested"] {
                let child = gui.child_by_name(owner, name).unwrap();
                commands.entity(child).insert(GuiTextVars::new([
                    ("name", "FairyGUI 中文"),
                    ("item", "<i>Template</i>"),
                ]));
            }
            next = true;
        }
        3 if elapsed >= 12 => {
            let owner = state.owner.unwrap();
            let plain = gui.child_by_name(owner, "plain").unwrap();
            let input = gui.child_by_name(owner, "input").unwrap();
            let rich = gui.child_by_name(owner, "rich").unwrap();
            let nested = gui.child_by_name(owner, "nested").unwrap();
            let title = gui.child_by_name(nested, "title").unwrap();
            assert_eq!(text.get(plain).unwrap().0, "Hello FairyGUI 中文! ");
            assert_eq!(text.get(input).unwrap().0, "FairyGUI 中文");
            assert_eq!(text.get(title).unwrap().0, "Nested FairyGUI 中文");
            assert_eq!(
                span_text(rich, &spans, &children),
                "FairyGUI 中文 / Template"
            );
            assert!(gui.transform(plain).unwrap().size().x > state.plain_width);
            assert_eq!(gui.text(input), Some("{name=Guest}"));
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-text-vars-updated.png"));
            next = true;
        }
        4 if elapsed >= 8 => {
            let owner = state.owner.unwrap();
            let plain = gui.child_by_name(owner, "plain").unwrap();
            ui.get_mut(plain)
                .unwrap()
                .set_text_variables(GuiTextVars::default());
            let nested = gui.child_by_name(owner, "nested").unwrap();
            let mut empty = GuiTextVars::default();
            empty.clear();
            ui.get_mut(nested).unwrap().set_text_variables(empty);
            next = true;
        }
        5 if elapsed >= 10 => {
            let owner = state.owner.unwrap();
            let plain = gui.child_by_name(owner, "plain").unwrap();
            let nested = gui.child_by_name(owner, "nested").unwrap();
            let title = gui.child_by_name(nested, "title").unwrap();
            assert_eq!(text.get(plain).unwrap().0, "Hello {name=Guest}! {missing}");
            assert_eq!(text.get(title).unwrap().0, "Nested Guest");
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-text-vars-disabled.png"));
            next = true;
        }
        6 if elapsed >= 8 => {
            commands.entity(state.owner.unwrap()).despawn();
            next = true;
        }
        7 if elapsed >= 8 => {
            assert!(gui.view(state.owner.unwrap()).is_none());
            info!(
                "demo smoke PASSED: exported template defaults, ECS variables, raw source retention, rich markup, input display, nested titles, autosize, disable/clear and disposal"
            );
            exit.write(AppExit::Success);
            next = true;
        }
        _ => {}
    }
    if next {
        state.step += 1;
        state.since = state.frame;
    }
}
fn span_text(entity: Entity, spans: &Query<&TextSpan>, children: &Query<&Children>) -> String {
    let mut value = spans
        .get(entity)
        .map_or(String::new(), |span| span.0.clone());
    if let Ok(children_list) = children.get(entity) {
        for child in children_list.iter() {
            value.push_str(&span_text(child, spans, children));
        }
    }
    value
}
