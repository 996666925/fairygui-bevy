//! Real rendering and rectangle picking through mutable visual properties.
use super::*;

#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    step: u8,
    page: Option<Entity>,
    image: Option<Entity>,
    loader: Option<Entity>,
    content: Option<Entity>,
    cursor: Option<Vec2>,
}
#[derive(Resource, Default)]
struct Clicks(u32);
fn clicked(event: On<GuiEvent>, mut clicks: ResMut<Clicks>) {
    if event.event.kind == fairygui_core::object::EventKind::Click && !event.event.bubbling {
        clicks.0 += 1;
    }
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<State>()
        .init_resource::<Clicks>()
        .add_systems(
            PreUpdate,
            drive
                .after(InputSystems)
                .before(UiSystems::Focus)
                .before(PickingSystems::ProcessInput),
        );
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn drive(
    mut state: ResMut<State>,
    clicks: Res<Clicks>,
    demo: Res<Demo>,
    mut ui: ParamSet<(
        GuiQuery,
        Query<&mut GuiLoaderSettings>,
        Query<&mut GuiImageFill>,
        Query<&mut GuiColor>,
        Query<&mut GuiIcon>,
        Query<&mut GuiTransform>,
        Query<&mut GuiDisplay>,
    )>,
    assets: Res<AssetServer>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    children: Query<&Children>,
    images: Query<&ImageNode>,
    image_assets: Res<Assets<Image>>,
    entities: Query<()>,
    mut windows: Query<(Entity, &mut Window), With<bevy::window::PrimaryWindow>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    assert!(
        state.frame < 1600,
        "visual-properties smoke timed out at step {}",
        state.step
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    let elapsed = state.frame - state.since;
    let mut next = false;
    match state.step {
        0 if state.frame >= 30 => {
            let Some(menu) = demo.menu_owner.and_then(|owner| ui.p0().view(owner)) else {
                return;
            };
            ui.p6().get_mut(menu).unwrap().visible = false;
            let asset = assets.load("ui/Basics.fui");
            state.page = Some(
                commands
                    .spawn(
                        FairyGuiRoot::new(asset.clone(), "Demo_Loader")
                            .with_position(Vec2::new(60., 80.)),
                    )
                    .id(),
            );
            state.image = Some(
                commands
                    .spawn((
                        FairyGuiRoot::new(asset, "pic")
                            .with_position(Vec2::new(760., 160.))
                            .with_size(Vec2::new(220., 220.)),
                        GuiColor(Color::srgb(0.4, 0.8, 1.)),
                        GuiImageFill {
                            method: FillMethod::Radial360,
                            amount: 0.65,
                            clockwise: true,
                            ..default()
                        },
                    ))
                    .id(),
            );
            next = true;
        }
        1 if elapsed >= 16
            && ui.p0().view(state.page.unwrap()).is_some()
            && ui.p0().view(state.image.unwrap()).is_some() =>
        {
            let (loader, all) = {
                let gui = ui.p0();
                (
                    gui.child_by_name(state.page.unwrap(), "n2").unwrap(),
                    gui.get(state.page.unwrap()).unwrap().children().to_vec(),
                )
            };
            for child in all {
                ui.p6().get_mut(child).unwrap().visible = child == loader;
            }
            {
                let mut transforms = ui.p5();
                let mut transform = transforms.get_mut(loader).unwrap();
                transform.position = Vec2::new(100., 80.);
                transform.size = Vec2::new(301., 201.);
            }
            *ui.p1().get_mut(loader).unwrap() = GuiLoaderSettings {
                fill: 4,
                align: 2,
                vertical_align: 2,
                ..default()
            };
            *ui.p2().get_mut(loader).unwrap() = GuiImageFill {
                method: FillMethod::Horizontal,
                amount: 0.5,
                ..default()
            };
            ui.p3().get_mut(loader).unwrap().0 = Color::srgb(1., 0.4, 0.4);
            ui.p4().get_mut(loader).unwrap().set("ui://Basics/pic");
            commands.entity(loader).observe(clicked);
            state.loader = Some(loader);
            next = true;
        }
        2 if elapsed >= 16 => {
            let loader = state.loader.unwrap();
            assert_eq!(ui.p0().loader(loader), Some(&GuiLoaderState::Ready));
            assert_eq!(
                ui.p0().transform(loader).unwrap().size,
                Vec2::new(301., 201.)
            );
            let visual = if images.get(loader).is_ok() {
                loader
            } else {
                children
                    .get(loader)
                    .unwrap()
                    .iter()
                    .find(|&child| images.get(child).is_ok())
                    .unwrap()
            };
            assert!(
                image_assets
                    .get(&images.get(visual).unwrap().image)
                    .is_some()
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-visual-properties-masks.png",
                ));
            // The right half has no painted pixels; the loader's original
            // rectangle must still receive its event there.
            let (node, transform) = nodes.get(loader).unwrap();
            state.cursor = Some(
                transform.transform_point2(Vec2::new(node.size().x * 0.4, 0.))
                    / window.scale_factor(),
            );
            next = true;
        }
        3 if elapsed >= 6 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        4 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        5 if elapsed >= 5 => {
            assert_eq!(
                clicks.0, 1,
                "masked-away pixels preserve the native input rectangle"
            );
            let loader = state.loader.unwrap();
            ui.p2().get_mut(loader).unwrap().amount = 1.;
            ui.p3().get_mut(loader).unwrap().0 = Color::srgb(0.4, 1., 0.4);
            ui.p1().get_mut(loader).unwrap().fill = 1;
            next = true;
        }
        6 if elapsed >= 16 => {
            let loader = state.loader.unwrap();
            assert_eq!(ui.p0().loader_settings(loader).unwrap().fill, 1);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-visual-properties-fit.png",
                ));
            next = true;
        }
        7 if elapsed >= 6 => {
            let loader = state.loader.unwrap();
            ui.p4()
                .get_mut(loader)
                .unwrap()
                .set("ui://Basics/Component12");
            {
                let mut settings = ui.p1();
                let mut settings = settings.get_mut(loader).unwrap();
                settings.fill = 4;
                settings.use_resize = true;
            }
            ui.p2().get_mut(loader).unwrap().amount = 0.25;
            next = true;
        }
        8 if elapsed >= 16 => {
            let loader = state.loader.unwrap();
            let content = *ui.p0().get(loader).unwrap().children().first().unwrap();
            assert!(
                ui.p0()
                    .transform(content)
                    .unwrap()
                    .size
                    .abs_diff_eq(Vec2::new(301., 201.), 0.001)
            );
            let background = ui.p0().child_by_name(content, "n9").unwrap();
            assert!(
                ui.p0()
                    .transform(background)
                    .unwrap()
                    .size
                    .abs_diff_eq(Vec2::new(301., 201.), 0.001)
            );
            state.content = Some(content);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-visual-properties-component.png",
                ));
            next = true;
        }
        9 if elapsed >= 6 => {
            ui.p1().get_mut(state.loader.unwrap()).unwrap().auto_size = true;
            next = true;
        }
        10 if elapsed >= 16 => {
            let loader = state.loader.unwrap();
            assert_eq!(
                ui.p0().transform(loader).unwrap().size,
                Vec2::new(400., 297.)
            );
            assert!(
                ui.p0()
                    .get(loader)
                    .unwrap()
                    .children()
                    .contains(&state.content.unwrap())
            );
            ui.p1().get_mut(loader).unwrap().auto_size = false;
            ui.p4().get_mut(loader).unwrap().set("ui://Basics/pic");
            next = true;
        }
        11 if elapsed >= 16 => {
            assert!(!entities.contains(state.content.unwrap()));
            assert_eq!(
                ui.p0().loader(state.loader.unwrap()),
                Some(&GuiLoaderState::Ready)
            );
            commands.entity(state.page.unwrap()).despawn();
            commands.entity(state.image.unwrap()).despawn();
            next = true;
        }
        12 if elapsed >= 6 => {
            assert!(!entities.contains(state.loader.unwrap()));
            assert!(ui.p0().view(state.image.unwrap()).is_none());
            println!(
                "demo smoke PASSED: writable image/loader color and fill masks, original rectangle picking, fit/stretch, resized component relations, automatic size feedback, retained loader identity, URL replacement and disposal"
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
    window.set_cursor_position(state.cursor);
    move_pointer(window_entity, state.cursor, &mut pointers);
}
