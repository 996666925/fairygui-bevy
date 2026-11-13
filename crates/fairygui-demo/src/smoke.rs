//! Optional end-to-end demo check using actual Bevy layout and focus handling.
use super::*;
use bevy::input::InputSystems;
use bevy::picking::{
    PickingSystems,
    pointer::{Location, PointerAction, PointerId, PointerInput},
};
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::ui::UiSystems;

#[path = "smoke_bitmap_rich_text.rs"]
mod bitmap_rich_text;
#[path = "smoke_bitmap_text.rs"]
mod bitmap_text;
#[path = "smoke_button_fire.rs"]
mod button_fire;
#[path = "smoke_color_filters.rs"]
mod color_filters;
#[path = "smoke_compositor.rs"]
mod compositor;
#[path = "smoke_external_inline.rs"]
mod external_inline;
#[path = "smoke_fills.rs"]
mod fills;
#[path = "smoke_gear_updates.rs"]
mod gear_updates;
#[path = "smoke_grayed.rs"]
mod grayed;
#[path = "smoke_input_events.rs"]
mod input_events;
#[path = "smoke_input_policy.rs"]
mod input_policy;
#[path = "smoke_list_bars.rs"]
mod list_bars;
#[path = "smoke_list_selection.rs"]
mod list_selection;
#[path = "smoke_loader_components.rs"]
mod loader_components;
#[path = "smoke_loop_list.rs"]
mod loop_list;
#[path = "smoke_masks.rs"]
mod masks;
#[path = "smoke_nine_grid.rs"]
mod nine_grid;
#[path = "smoke_polygons.rs"]
mod polygons;
#[path = "smoke_popup_templates.rs"]
mod popup_templates;
#[path = "smoke_pull_refresh.rs"]
mod pull_refresh;
#[path = "smoke_scroll_pages.rs"]
mod scroll_pages;
#[path = "smoke_scroll_view.rs"]
mod scroll_view;
#[path = "smoke_text_fonts.rs"]
mod text_fonts;
#[path = "smoke_text_image_colors.rs"]
mod text_image_colors;
#[path = "smoke_text_stroke.rs"]
mod text_stroke;
#[path = "smoke_virtual_list.rs"]
mod virtual_list;
#[path = "smoke_window_wait.rs"]
mod window_wait;
#[path = "smoke_windows.rs"]
mod windows;

pub(super) fn install_window_wait(app: &mut App) {
    window_wait::install(app);
}

pub(super) fn install_windows(app: &mut App) {
    windows::install(app);
}

#[derive(Resource, Default)]
struct Smoke {
    transition_only: bool,
    frames: u32,
    step: u32,
    entry: usize,
    since: u32,
    basics_entry: usize,
    visits: u8,
    cursor: Option<Vec2>,
    controller_stage: u8,
    transition_entry: usize,
    transition_stage: u8,
    controls_since: u32,
    controls_snapshot: Option<(f32, f32, Vec2, f32)>,
    interval_snapshot: Option<(f32, usize)>,
    movie_stage: u8,
    movie_object: Option<fairygui_core::object::GObjectId>,
    movie_completions: usize,
    movie_since: u32,
    edit_callbacks: usize,
    lifecycle_stage: u8,
    lifecycle_root: Option<usize>,
    lifecycle_entity: Option<Entity>,
    lifecycle_time: f32,
    lifecycle_since: u32,
    inline_stage: u8,
    inline_text: Option<GObjectId>,
    inline_since: u32,
    inline_original: Option<(String, Vec2)>,
    inline_images: Vec<Entity>,
    inline_container: Option<GObjectId>,
    inline_pool: fairygui_core::pool::ObjectPool,
    link_stage: u8,
    link_original: Option<String>,
    link_messages: usize,
    button_stage: u8,
    button_scale: Vec2,
    button_since: u32,
    popup_stage: u8,
    popup_root: Option<usize>,
    popup_entity: Option<Entity>,
    combo_stage: u8,
    combo_dropdown: Option<GObjectId>,
    combo_view: Option<Entity>,
    menu_stage: u8,
    menu_item: Option<GObjectId>,
    menu_view: Option<Entity>,
    menu_since: f32,
    cross_popups: popup_templates::State,
    loader_components: loader_components::State,
    polygons: polygons::State,
    fills: fills::State,
    nine_grid: nine_grid::State,
    color_filters: color_filters::State,
    grayed: grayed::State,
    gear_updates: gear_updates::State,
    bitmap_text: bitmap_text::State,
    text_image_colors: text_image_colors::State,
    text_fonts: text_fonts::State,
    bitmap_rich_text: bitmap_rich_text::State,
    external_inline: external_inline::State,
    input_policy: input_policy::State,
    input_events: input_events::State,
    text_stroke: text_stroke::State,
    list_selection: list_selection::State,
    scroll_view: scroll_view::State,
}

pub(super) fn install(app: &mut App) {
    app.add_systems(
        Update,
        fills::hold_progress
            .after(GuiSystems::Respond)
            .before(GuiSystems::Advance),
    );
    button_fire::install(app);
    input_events::install(app);
    list_selection::install(app);
    app.init_resource::<Smoke>().add_systems(
        PreUpdate,
        drive
            .after(InputSystems)
            .before(bevy::input_focus::InputFocusSystems::Dispatch)
            .before(UiSystems::Focus)
            .before(PickingSystems::ProcessInput),
    );
}

pub(super) fn install_transition(app: &mut App) {
    install(app);
    let mut smoke = app.world_mut().resource_mut::<Smoke>();
    smoke.entry = 1;
    smoke.transition_entry = 0;
    smoke.transition_only = true;
}

pub(super) fn install_scroll_view(app: &mut App) {
    scroll_view::install_focused(app);
}

pub(super) fn install_list_bars(app: &mut App) {
    list_bars::install(app);
}

pub(super) fn install_scroll_pages(app: &mut App) {
    scroll_pages::install(app);
}

pub(super) fn install_pull_refresh(app: &mut App) {
    pull_refresh::install(app);
}

pub(super) fn install_virtual_list(app: &mut App) {
    virtual_list::install(app);
}

pub(super) fn install_loop_list(app: &mut App) {
    loop_list::install(app);
}

pub(super) fn install_compositor(app: &mut App) {
    compositor::install(app);
}

pub(super) fn install_masks(app: &mut App) {
    masks::install(app);
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn drive(
    mut smoke: ResMut<Smoke>,
    demo: Res<Demo>,
    controller: Res<demos::DemoController>,
    mut runtime: NonSendMut<GuiRuntime>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    mut windows: Query<(Entity, &mut Window)>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
    mut pointers: MessageWriter<PointerInput>,
    (images, children, layout_nodes, inline_parts, parents, inline_boxes): (
        Query<&ImageNode>,
        Query<&Children>,
        Query<&Node>,
        Query<&fairygui_bevy::runtime::InlineImageParts>,
        Query<&ChildOf>,
        Query<&bevy::text::InlineBox>,
    ),
    visibility: Query<(&Visibility, &InheritedVisibility)>,
    glyphs: Query<&fairygui_bevy::runtime::BitmapGlyphs>,
    object_transforms: Query<&UiTransform>,
    movies: Query<&fairygui_bevy::movie::MovieClipPlayer>,
    (
        mut completed_movies,
        mut edited_hooks,
        mut assets,
        mut links,
        rich_layouts,
        text_spans,
        time,
        (input_state, global_wait),
        mut fire_smoke,
        mut image_assets,
        fonts,
        bitmap_metrics,
        bitmap_decorations,
        font_assets,
        asset_server,
        (editables, input_prompts, mut input_event_log, mut list_selection_log, scroll_positions),
    ): (
        MessageReader<fairygui_bevy::GuiAnimationComplete>,
        MessageReader<fairygui_bevy::GuiTransitionCallback>,
        ResMut<Assets<FuiAsset>>,
        MessageReader<GuiLink>,
        Query<(&bevy::text::ComputedTextBlock, &bevy::text::TextLayoutInfo)>,
        Query<&TextSpan>,
        Res<Time>,
        (Res<GuiInputState>, Res<GuiGlobalWaitState>),
        ResMut<button_fire::State>,
        ResMut<Assets<Image>>,
        Query<&TextFont>,
        Query<&fairygui_bevy::runtime::BitmapTextMetrics>,
        Query<&fairygui_bevy::runtime::BitmapDecorations>,
        Res<Assets<Font>>,
        Res<AssetServer>,
        (
            Query<&bevy::text::EditableText>,
            Query<(
                Entity,
                &fairygui_bevy::runtime::InputPromptOwner,
                &Visibility,
            )>,
            ResMut<input_events::Log>,
            ResMut<list_selection::Log>,
            Query<&ScrollPosition>,
        ),
    ),
) {
    smoke.frames += 1;
    for link in links.read() {
        assert_eq!(link.href, "event:xx");
        smoke.link_messages += 1;
    }
    for callback in edited_hooks.read() {
        if callback.callback.listener == 9001 {
            assert!(callback.callback.hook.at_end);
            smoke.edit_callbacks += 1;
        }
    }
    for completed in completed_movies.read() {
        if Some(completed.0) == smoke.movie_object {
            smoke.movie_completions += 1;
        }
    }
    if smoke.frames - smoke.since > 900 {
        eprintln!(
            "demo smoke FAILED: step={}, entry={}",
            smoke.step, smoke.entry
        );
        exit.write(AppExit::error());
        return;
    }
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    // Keep scripted input stable if the physical mouse moves during the run.
    window.set_cursor_position(smoke.cursor);
    let scale_factor = window.scale_factor();
    let (button, package) = catalog::ENTRIES[smoke.entry];
    match smoke.step {
        3 if smoke.entry == 0
            && demo.open.is_some()
            && smoke.basics_entry < demos::BASICS_PAGES.len()
            && smoke.frames - smoke.since > 30 =>
        {
            let root = controller.root.unwrap();
            let name = format!("btn_{}", demos::BASICS_PAGES[smoke.basics_entry]);
            let id = runtime
                .store()
                .find_by_name(root, &name)
                .expect("Basics menu button");
            let entity = runtime.entity_of(id).unwrap();
            let (_, transform) = nodes.get(entity).unwrap();
            smoke.cursor = Some(transform.transform_point2(Vec2::ZERO) / scale_factor);
            window.set_cursor_position(smoke.cursor);
            smoke.step = 7;
        }
        7 => {
            mouse.press(MouseButton::Left);
            smoke.step = 8;
        }
        8 => {
            mouse.release(MouseButton::Left);
            smoke.step = 9;
            smoke.since = smoke.frames;
        }
        9 if controller.basics_page.is_some() && smoke.frames - smoke.since > 30 => {
            let page = demos::BASICS_PAGES[smoke.basics_entry];
            if page == "List" && !smoke.scroll_view.complete() {
                let root = runtime
                    .root_object(controller.basics_page.unwrap())
                    .unwrap();
                let frames = smoke.frames;
                scroll_view::drive(
                    &mut smoke.scroll_view,
                    frames,
                    &mut runtime,
                    &assets,
                    &nodes,
                    &scroll_positions,
                    &mut window,
                    &mut mouse,
                    &mut commands,
                    root,
                );
                smoke.cursor = window.cursor_position();
                move_pointer(window_entity, smoke.cursor, &mut pointers);
                if smoke.scroll_view.complete() {
                    smoke.since = frames;
                }
                return;
            }
            if page == "List" && !smoke.list_selection.complete() {
                let root = runtime
                    .root_object(controller.basics_page.unwrap())
                    .unwrap();
                let frames = smoke.frames;
                list_selection::drive(
                    &mut smoke.list_selection,
                    frames,
                    &mut runtime,
                    &assets,
                    &nodes,
                    &mut window,
                    &mut mouse,
                    &mut commands,
                    &mut list_selection_log,
                    root,
                );
                smoke.cursor = window.cursor_position();
                move_pointer(window_entity, smoke.cursor, &mut pointers);
                if smoke.list_selection.complete() {
                    smoke.since = frames;
                }
                return;
            }
            if page == "Text" && !smoke.input_policy.complete() {
                let root = runtime
                    .root_object(controller.basics_page.unwrap())
                    .unwrap();
                let frames = smoke.frames;
                input_policy::drive(
                    &mut smoke.input_policy,
                    frames,
                    &mut runtime,
                    &mut assets,
                    &editables,
                    &rich_layouts,
                    &layout_nodes,
                    &input_prompts,
                    &mut commands,
                    root,
                );
                // Completed sub-suites are verified progress. Keep the watchdog
                // bounded per suite as Text accumulates additional regressions.
                if smoke.input_policy.complete() {
                    smoke.since = frames;
                }
                return;
            }
            if page == "Text" && !smoke.input_events.complete() {
                let root = runtime
                    .root_object(controller.basics_page.unwrap())
                    .unwrap();
                let frames = smoke.frames;
                input_events::drive(
                    &mut smoke.input_events,
                    &mut input_event_log,
                    frames,
                    &mut runtime,
                    &mut assets,
                    &editables,
                    &nodes,
                    &mut mouse,
                    window_entity,
                    &mut window,
                    &mut commands,
                    root,
                );
                smoke.cursor = window.cursor_position();
                move_pointer(window_entity, smoke.cursor, &mut pointers);
                if smoke.input_events.complete() {
                    smoke.since = frames;
                }
                return;
            }
            if page == "Text" && !smoke.text_stroke.complete() {
                let root = runtime
                    .root_object(controller.basics_page.unwrap())
                    .unwrap();
                let frames = smoke.frames;
                text_stroke::drive(
                    &mut smoke.text_stroke,
                    frames,
                    &mut runtime,
                    &mut assets,
                    &nodes,
                    &editables,
                    &mut commands,
                    root,
                );
                if smoke.text_stroke.complete() {
                    smoke.since = frames;
                }
                return;
            }
            if page == "Text" && !smoke.external_inline.complete() {
                let root = runtime
                    .root_object(controller.basics_page.unwrap())
                    .unwrap();
                let frames = smoke.frames;
                external_inline::drive(
                    &mut smoke.external_inline,
                    frames,
                    &mut runtime,
                    &mut assets,
                    &image_assets,
                    &inline_parts,
                    &glyphs,
                    &children,
                    &images,
                    &nodes,
                    &layout_nodes,
                    &mut commands,
                    root,
                );
                if smoke.external_inline.complete() {
                    smoke.since = frames;
                }
                return;
            }
            if page == "Text" && !smoke.bitmap_rich_text.complete() {
                let root = runtime
                    .root_object(controller.basics_page.unwrap())
                    .unwrap();
                let frames = smoke.frames;
                bitmap_rich_text::drive(
                    &mut smoke.bitmap_rich_text,
                    frames,
                    &mut runtime,
                    &mut assets,
                    &glyphs,
                    &images,
                    &children,
                    &layout_nodes,
                    &mut commands,
                    root,
                );
                if smoke.bitmap_rich_text.complete() {
                    smoke.since = frames;
                }
                return;
            }
            if page == "Text" && !smoke.text_fonts.complete() {
                let root = runtime
                    .root_object(controller.basics_page.unwrap())
                    .unwrap();
                let frames = smoke.frames;
                text_fonts::drive(
                    &mut smoke.text_fonts,
                    frames,
                    &mut runtime,
                    &mut assets,
                    &glyphs,
                    &inline_parts,
                    &images,
                    &children,
                    &text_spans,
                    &layout_nodes,
                    &font_assets,
                    &asset_server,
                    &fonts,
                    &rich_layouts,
                    &editables,
                    &mut commands,
                    root,
                );
                if smoke.text_fonts.complete() {
                    smoke.since = frames;
                }
                return;
            }
            if page == "Text" && !smoke.text_image_colors.complete() {
                let root = runtime
                    .root_object(controller.basics_page.unwrap())
                    .unwrap();
                let frames = smoke.frames;
                text_image_colors::drive(
                    &mut smoke.text_image_colors,
                    frames,
                    &mut runtime,
                    &mut assets,
                    &mut image_assets,
                    &glyphs,
                    &inline_parts,
                    &images,
                    &nodes,
                    &layout_nodes,
                    &mut commands,
                    root,
                );
                if smoke.text_image_colors.complete() {
                    smoke.since = frames;
                }
                return;
            }
            if page == "Text" && !smoke.bitmap_text.complete() {
                let root = runtime
                    .root_object(controller.basics_page.unwrap())
                    .unwrap();
                let frames = smoke.frames;
                bitmap_text::drive(
                    &mut smoke.bitmap_text,
                    frames,
                    &mut runtime,
                    &mut assets,
                    &glyphs,
                    &bitmap_metrics,
                    &bitmap_decorations,
                    &layout_nodes,
                    &mut commands,
                    root,
                );
                if smoke.bitmap_text.complete() {
                    smoke.since = frames;
                }
                return;
            }
            if page == "Controller" && !smoke.gear_updates.complete() {
                let root = runtime
                    .root_object(controller.basics_page.unwrap())
                    .unwrap();
                let frames = smoke.frames;
                gear_updates::drive(
                    &mut smoke.gear_updates,
                    frames,
                    &mut runtime,
                    &mut assets,
                    &layout_nodes,
                    &fonts,
                    &nodes,
                    &mut commands,
                    root,
                );
                return;
            }
            if matches!(page, "Component" | "Button") && !smoke.grayed.complete_for(page) {
                let root = runtime
                    .root_object(controller.basics_page.unwrap())
                    .unwrap();
                let frames = smoke.frames;
                grayed::drive(
                    &mut smoke.grayed,
                    frames,
                    &mut runtime,
                    &mut assets,
                    &nodes,
                    &mut commands,
                    root,
                    page,
                );
                return;
            }
            if page == "Image" && !smoke.color_filters.complete() {
                let root = runtime
                    .root_object(controller.basics_page.unwrap())
                    .unwrap();
                let frames = smoke.frames;
                color_filters::drive(
                    &mut smoke.color_filters,
                    frames,
                    &mut runtime,
                    &mut assets,
                    &mut image_assets,
                    &images,
                    &nodes,
                    &mut commands,
                    root,
                );
                return;
            }
            if page == "Image" && !smoke.nine_grid.complete() {
                let root = runtime
                    .root_object(controller.basics_page.unwrap())
                    .unwrap();
                let frames = smoke.frames;
                nine_grid::drive(
                    &mut smoke.nine_grid,
                    frames,
                    &mut runtime,
                    &mut assets,
                    &mut image_assets,
                    &images,
                    &children,
                    &mut commands,
                    root,
                );
                return;
            }
            if page == "Graph" && !smoke.polygons.complete() {
                let root = runtime
                    .root_object(controller.basics_page.unwrap())
                    .unwrap();
                let frames = smoke.frames;
                polygons::drive(
                    &mut smoke.polygons,
                    frames,
                    &mut runtime,
                    &nodes,
                    &mut commands,
                    &mut mouse,
                    &mut window,
                    root,
                );
                smoke.cursor = window.cursor_position();
                move_pointer(window_entity, smoke.cursor, &mut pointers);
                return;
            }
            if page == "ComboBox" && smoke.combo_stage < 13 {
                let root = runtime
                    .root_object(controller.basics_page.unwrap())
                    .unwrap();
                let combo = runtime.store().child_by_name(root, "n1").unwrap();
                match smoke.combo_stage {
                    0 | 6 => {
                        if smoke.combo_stage == 6 {
                            assert!(runtime.popup_stack().is_empty());
                            assert!(matches!(runtime.store().object(combo).unwrap().control(),
                                fairygui_core::widget::ControlData::ComboBox(data) if data.selected_index == 2));
                        }
                        let (_, transform) = nodes.get(runtime.entity_of(combo).unwrap()).unwrap();
                        smoke.cursor = Some(transform.transform_point2(Vec2::ZERO) / scale_factor);
                        window.set_cursor_position(smoke.cursor);
                        smoke.button_since = smoke.frames;
                        smoke.combo_stage += 1;
                    }
                    1 | 4 | 7 | 10 if smoke.frames - smoke.button_since >= 4 => {
                        mouse.press(MouseButton::Left);
                        smoke.button_since = smoke.frames;
                        smoke.combo_stage += 1;
                    }
                    2 if smoke.frames - smoke.button_since >= 8 => {
                        let popup = runtime.store().combo_popup(combo).unwrap();
                        smoke.combo_dropdown = Some(popup.dropdown);
                        smoke.combo_view = runtime.entity_of(popup.dropdown);
                        assert_eq!(runtime.popup_stack(), &[popup.dropdown]);
                        let state = runtime
                            .store()
                            .object(combo)
                            .unwrap()
                            .control()
                            .component()
                            .unwrap()
                            .controller("button")
                            .unwrap()
                            .selected_page_name();
                        assert_eq!(state, Some("down"));
                        commands
                            .spawn(Screenshot::primary_window())
                            .observe(save_to_disk("target/fairygui-demo-combo-dropdown.png"));
                        mouse.release(MouseButton::Left);
                        smoke.button_since = smoke.frames;
                        smoke.combo_stage = 3;
                    }
                    3 | 9 if smoke.frames - smoke.button_since >= 4 => {
                        let popup = runtime.store().combo_popup(combo).unwrap();
                        let item = runtime.store().child_at(popup.list, 2).unwrap();
                        let (_, transform) = nodes.get(runtime.entity_of(item).unwrap()).unwrap();
                        smoke.cursor = Some(transform.transform_point2(Vec2::ZERO) / scale_factor);
                        window.set_cursor_position(smoke.cursor);
                        smoke.button_since = smoke.frames;
                        smoke.combo_stage += 1;
                    }
                    5 | 11 => {
                        if smoke.combo_stage == 11 {
                            assert!(
                                !runtime.is_popup_menu_open(),
                                "row press closes only the menu above the dropdown"
                            );
                            assert!(runtime.store().combo_popup(combo).is_some());
                        }
                        mouse.release(MouseButton::Left);
                        smoke.button_since = smoke.frames;
                        smoke.combo_stage += 1;
                    }
                    8 if smoke.frames - smoke.button_since >= 8 => {
                        let popup = runtime.store().combo_popup(combo).unwrap();
                        assert_eq!(smoke.combo_dropdown, Some(popup.dropdown));
                        assert_eq!(smoke.combo_view, runtime.entity_of(popup.dropdown));
                        let handle = runtime.asset_handle_for(root).unwrap();
                        let asset = assets.get(&handle).unwrap();
                        let menu = runtime
                            .show_popup_menu(
                                &mut commands,
                                &asset.package,
                                "PopupMenu",
                                "PopupMenuItem",
                                &["First", "Second", "Third"],
                                fairygui_core::geom::Point::new(600.0, 230.0),
                            )
                            .unwrap();
                        assert_eq!(runtime.popup_stack(), &[popup.dropdown, menu]);
                        mouse.release(MouseButton::Left);
                        smoke.button_since = smoke.frames;
                        smoke.combo_stage = 9;
                        commands
                            .spawn(Screenshot::primary_window())
                            .observe(save_to_disk("target/fairygui-demo-popup-mixed.png"));
                    }
                    12 if smoke.frames - smoke.button_since >= 8 => {
                        assert!(runtime.popup_stack().is_empty());
                        let popup = smoke.combo_dropdown.unwrap();
                        assert!(runtime.store().contains(popup));
                        assert_eq!(runtime.entity_of(popup), smoke.combo_view);
                        assert!(!visibility.get(smoke.combo_view.unwrap()).unwrap().1.get());
                        smoke.combo_stage = 13;
                        println!(
                            "demo smoke: combo opened on down, selected and reselected row, retained dropdown/items and closed a mixed menu/dropdown stack"
                        );
                    }
                    _ => {}
                }
                move_pointer(window_entity, window.cursor_position(), &mut pointers);
                return;
            }
            if page == "ComboBox" && !smoke.cross_popups.complete() {
                let root = runtime
                    .root_object(controller.basics_page.unwrap())
                    .unwrap();
                let frames = smoke.frames;
                popup_templates::drive(
                    &mut smoke.cross_popups,
                    frames,
                    &mut runtime,
                    &mut assets,
                    &nodes,
                    &mut window,
                    &mut commands,
                    &mut mouse,
                    root,
                );
                smoke.cursor = window.cursor_position();
                move_pointer(window_entity, smoke.cursor, &mut pointers);
                return;
            }
            if page == "Loader" && !smoke.loader_components.complete() {
                let root = runtime
                    .root_object(controller.basics_page.unwrap())
                    .unwrap();
                let frames = smoke.frames;
                loader_components::drive(
                    &mut smoke.loader_components,
                    frames,
                    &mut runtime,
                    &nodes,
                    &images,
                    &mut commands,
                    &mut mouse,
                    &mut window,
                    root,
                );
                smoke.cursor = window.cursor_position();
                move_pointer(window_entity, smoke.cursor, &mut pointers);
                return;
            }
            if page == "Button" && smoke.button_stage < 8 {
                let root = runtime
                    .root_object(controller.basics_page.unwrap())
                    .unwrap();
                let button = runtime.store().child_by_name(root, "n13").unwrap();
                let entity = runtime.entity_of(button).unwrap();
                let background = runtime.store().child_by_name(button, "bg").unwrap();
                match smoke.button_stage {
                    0 => {
                        let object = runtime.store().object(button).unwrap();
                        assert_eq!(object.pivot(), fairygui_core::geom::Point::new(0.5, 0.5));
                        smoke.button_scale = Vec2::new(object.scale_x(), object.scale_y());
                        let (_, transform) = nodes.get(entity).unwrap();
                        smoke.cursor = Some(transform.transform_point2(Vec2::ZERO) / scale_factor);
                        window.set_cursor_position(smoke.cursor);
                        smoke.button_since = smoke.frames;
                        smoke.button_stage = 1;
                    }
                    1 | 5 if smoke.frames - smoke.button_since >= 4 => {
                        mouse.press(MouseButton::Left);
                        smoke.button_since = smoke.frames;
                        smoke.button_stage += 1;
                    }
                    2 if smoke.frames - smoke.button_since >= 8 => {
                        let object = runtime.store().object(button).unwrap();
                        let expected = smoke.button_scale * 0.8;
                        assert!(
                            Vec2::new(object.scale_x(), object.scale_y())
                                .abs_diff_eq(expected, 0.001)
                        );
                        assert!(
                            object_transforms
                                .get(entity)
                                .unwrap()
                                .scale
                                .abs_diff_eq(expected, 0.001)
                        );
                        commands
                            .spawn(Screenshot::primary_window())
                            .observe(save_to_disk("target/fairygui-demo-button-down.png"));
                        smoke.cursor = Some(Vec2::new(5.0, 5.0));
                        window.set_cursor_position(smoke.cursor);
                        smoke.button_since = smoke.frames;
                        smoke.button_stage = 3;
                    }
                    3 if smoke.frames - smoke.button_since >= 8 => {
                        let object = runtime.store().object(button).unwrap();
                        assert!((object.scale_x() - smoke.button_scale.x * 0.8).abs() < 0.001);
                        mouse.release(MouseButton::Left);
                        smoke.button_since = smoke.frames;
                        smoke.button_stage = 4;
                    }
                    4 if smoke.frames - smoke.button_since >= 8 => {
                        let object = runtime.store().object(button).unwrap();
                        assert!((object.scale_x() - smoke.button_scale.x).abs() < 0.001);
                        assert!(runtime.store_mut().set_button_down_effect(button, 1, 0.6));
                        let (_, transform) = nodes.get(entity).unwrap();
                        smoke.cursor = Some(transform.transform_point2(Vec2::ZERO) / scale_factor);
                        window.set_cursor_position(smoke.cursor);
                        smoke.button_since = smoke.frames;
                        smoke.button_stage = 5;
                    }
                    6 if smoke.frames - smoke.button_since >= 8 => {
                        let color = images
                            .get(runtime.entity_of(background).unwrap())
                            .unwrap()
                            .color
                            .to_srgba();
                        assert!((color.red - 0.6).abs() < 0.001);
                        commands
                            .spawn(Screenshot::primary_window())
                            .observe(save_to_disk("target/fairygui-demo-button-tint.png"));
                        mouse.release(MouseButton::Left);
                        smoke.button_since = smoke.frames;
                        smoke.button_stage = 7;
                    }
                    7 if smoke.frames - smoke.button_since >= 8 => {
                        let color = images
                            .get(runtime.entity_of(background).unwrap())
                            .unwrap()
                            .color
                            .to_srgba();
                        assert!((color.red - 1.0).abs() < 0.001);
                        runtime.store_mut().set_button_down_effect(button, 2, 0.8);
                        smoke.button_stage = 8;
                        println!(
                            "demo smoke: exported center pivot, scale while held outside, release restoration, pressed tint and white restoration verified"
                        );
                    }
                    _ => {}
                }
                move_pointer(window_entity, window.cursor_position(), &mut pointers);
                return;
            }
            if page == "Button" && !fire_smoke.complete() {
                let root = runtime
                    .root_object(controller.basics_page.unwrap())
                    .unwrap();
                smoke.cursor = Some(Vec2::new(5.0, 5.0));
                window.set_cursor_position(smoke.cursor);
                button_fire::drive(
                    &mut fire_smoke,
                    &mut runtime,
                    &object_transforms,
                    &mut commands,
                    root,
                );
                move_pointer(window_entity, smoke.cursor, &mut pointers);
                return;
            }
            if page == "Popup" && smoke.popup_stage < 10 {
                let root = runtime
                    .root_object(controller.basics_page.unwrap())
                    .unwrap();
                let button = runtime.store().child_by_name(root, "n1").unwrap();
                match smoke.popup_stage {
                    0 => {
                        let handle = runtime.asset_handle_for(root).unwrap();
                        let asset = assets.get(&handle).unwrap();
                        let index = runtime
                            .create_root(handle, &asset.package, "Component12")
                            .unwrap();
                        runtime.set_root_attached(index, false);
                        let popup = runtime.root_object(index).unwrap();
                        runtime
                            .store_mut()
                            .set_button_linked_popup(button, Some(popup));
                        smoke.popup_root = Some(index);
                        let (_, transform) = nodes.get(runtime.entity_of(button).unwrap()).unwrap();
                        smoke.cursor = Some(transform.transform_point2(Vec2::ZERO) / scale_factor);
                        window.set_cursor_position(smoke.cursor);
                        smoke.button_since = smoke.frames;
                        smoke.popup_stage = 1;
                    }
                    1 | 3 | 5 | 8 if smoke.frames - smoke.button_since >= 4 => {
                        mouse.press(MouseButton::Left);
                        smoke.button_since = smoke.frames;
                        smoke.popup_stage += 1;
                    }
                    2 | 6 if smoke.frames - smoke.button_since >= 8 => {
                        let popup = runtime.root_object(smoke.popup_root.unwrap()).unwrap();
                        assert_eq!(
                            runtime.popup_stack(),
                            &[popup],
                            "popup stage {}, cursor {:?}, root {:?}",
                            smoke.popup_stage,
                            smoke.cursor,
                            runtime.root_container()
                        );
                        let entity = runtime.entity_of(popup).unwrap();
                        assert!(visibility.get(entity).unwrap().1.get());
                        if smoke.popup_stage == 2 {
                            smoke.popup_entity = Some(entity);
                            commands
                                .spawn(Screenshot::primary_window())
                                .observe(save_to_disk("target/fairygui-demo-button-popup.png"));
                        } else {
                            assert_eq!(smoke.popup_entity, Some(entity));
                        }
                        mouse.release(MouseButton::Left);
                        smoke.button_since = smoke.frames;
                        smoke.popup_stage += 1;
                    }
                    4 if smoke.frames - smoke.button_since >= 8 => {
                        assert!(runtime.popup_stack().is_empty());
                        let entity = smoke.popup_entity.unwrap();
                        assert!(!visibility.get(entity).unwrap().1.get());
                        mouse.release(MouseButton::Left);
                        smoke.button_since = smoke.frames;
                        smoke.popup_stage = 5;
                    }
                    7 if smoke.frames - smoke.button_since >= 4 => {
                        smoke.cursor = Some(Vec2::new(5.0, 5.0));
                        window.set_cursor_position(smoke.cursor);
                        smoke.button_since = smoke.frames;
                        smoke.popup_stage = 8;
                    }
                    9 if smoke.frames - smoke.button_since >= 8 => {
                        assert!(runtime.popup_stack().is_empty());
                        mouse.release(MouseButton::Left);
                        runtime.store_mut().set_button_linked_popup(button, None);
                        runtime.remove_root(&mut commands, smoke.popup_root.take().unwrap());
                        smoke.popup_stage = 10;
                        println!(
                            "demo smoke: linked popup opened on press, closed without reopening, reused its view and closed on outside press"
                        );
                    }
                    _ => {}
                }
                move_pointer(window_entity, window.cursor_position(), &mut pointers);
                return;
            }
            if page == "Popup" && smoke.menu_stage < 17 {
                let root = runtime
                    .root_object(controller.basics_page.unwrap())
                    .unwrap();
                let button = runtime.store().child_by_name(root, "n0").unwrap();
                match smoke.menu_stage {
                    0 | 10 => {
                        let (_, transform) = nodes.get(runtime.entity_of(button).unwrap()).unwrap();
                        smoke.cursor = Some(transform.transform_point2(Vec2::ZERO) / scale_factor);
                        window.set_cursor_position(smoke.cursor);
                        smoke.button_since = smoke.frames;
                        smoke.menu_stage += 1;
                    }
                    1 | 4 | 7 | 11 | 14 if smoke.frames - smoke.button_since >= 4 => {
                        if smoke.menu_stage == 4 {
                            assert_eq!(
                                input_state.hovered(),
                                runtime.popup_menu_item("disabled").map(|item| item.1),
                                "later-created page must not intercept a popup row"
                            );
                        }
                        mouse.press(MouseButton::Left);
                        smoke.button_since = smoke.frames;
                        smoke.menu_stage += 1;
                    }
                    2 | 5 | 8 | 12 | 15 if smoke.frames - smoke.button_since >= 4 => {
                        mouse.release(MouseButton::Left);
                        smoke.menu_since = time.elapsed_secs();
                        smoke.button_since = smoke.frames;
                        smoke.menu_stage += 1;
                    }
                    3 if smoke.frames - smoke.button_since >= 8 => {
                        let menu = runtime.popup_menu_content().unwrap();
                        let list = runtime.store().child_by_name(menu, "list").unwrap();
                        runtime
                            .store_mut()
                            .set_list_fold_invisible_items(list, true);
                        runtime.update_popup_menu_item("first", |item| item.visible = false);
                        runtime.update_popup_menu_item("toggle", |item| {
                            item.caption = "Toggle option".into()
                        });
                        runtime
                            .reopen_popup_menu(fairygui_core::geom::Point::new(180.0, 210.0))
                            .unwrap();
                        let row = runtime.popup_menu_item("toggle").unwrap().1;
                        smoke.menu_item = Some(row);
                        smoke.menu_view = runtime.entity_of(row);
                        smoke.menu_stage = 4;
                        // Wait one rendered frame before measuring the edited rows.
                        smoke.button_since = smoke.frames;
                        let disabled = runtime.popup_menu_item("disabled").unwrap().1;
                        let (_, transform) =
                            nodes.get(runtime.entity_of(disabled).unwrap()).unwrap();
                        // Position is corrected below once the layout has synchronized.
                        smoke.cursor = Some(transform.transform_point2(Vec2::ZERO) / scale_factor);
                        window.set_cursor_position(smoke.cursor);
                    }
                    4 if smoke.frames - smoke.button_since < 4 => {
                        let disabled = runtime.popup_menu_item("disabled").unwrap().1;
                        let (_, transform) =
                            nodes.get(runtime.entity_of(disabled).unwrap()).unwrap();
                        smoke.cursor = Some(transform.transform_point2(Vec2::ZERO) / scale_factor);
                        window.set_cursor_position(smoke.cursor);
                        if smoke.frames - smoke.button_since == 3 {
                            commands
                                .spawn(Screenshot::primary_window())
                                .observe(save_to_disk("target/fairygui-demo-menu-actions.png"));
                        }
                    }
                    6 if time.elapsed_secs() - smoke.menu_since >= 0.15 => {
                        assert!(
                            runtime.is_popup_menu_open(),
                            "menu stage={}, cursor={:?}, hovered={:?}, action={:?}",
                            smoke.menu_stage,
                            smoke.cursor,
                            input_state.hovered(),
                            controller.last_menu_action
                        );
                        assert!(
                            controller.last_menu_action.is_none(),
                            "disabled row does not publish"
                        );
                        let row = smoke.menu_item.unwrap();
                        let (_, transform) = nodes.get(runtime.entity_of(row).unwrap()).unwrap();
                        smoke.cursor = Some(transform.transform_point2(Vec2::ZERO) / scale_factor);
                        window.set_cursor_position(smoke.cursor);
                        smoke.button_since = smoke.frames;
                        smoke.menu_stage = 7;
                    }
                    9 if time.elapsed_secs() - smoke.menu_since >= 0.15 => {
                        let action = controller
                            .last_menu_action
                            .as_ref()
                            .expect("application received menu action");
                        assert_eq!(action.action, Some(demos::BASICS_MENU_ACTION + 1));
                        assert_eq!(action.name, "toggle");
                        assert_eq!(action.caption, "Toggle option");
                        assert!(action.checked);
                        assert!(action.pointer.is_some());
                        assert!(!runtime.is_popup_menu_open());
                        let title = runtime.store().child_by_name(button, "title").unwrap();
                        assert_eq!(
                            runtime
                                .store()
                                .object(title)
                                .unwrap()
                                .control()
                                .as_text_field()
                                .unwrap()
                                .text,
                            "Toggle option ✓"
                        );
                        smoke.menu_stage = 10;
                    }
                    13 if smoke.frames - smoke.button_since >= 8 => {
                        assert!(runtime.is_popup_menu_open());
                        let (item, row) = runtime.popup_menu_item("toggle").unwrap();
                        assert!(item.checked);
                        assert_eq!(item.caption, "Toggle option");
                        assert_eq!(Some(row), smoke.menu_item);
                        assert_eq!(runtime.entity_of(row), smoke.menu_view);
                        let (_, transform) = nodes.get(runtime.entity_of(row).unwrap()).unwrap();
                        smoke.cursor = Some(transform.transform_point2(Vec2::ZERO) / scale_factor);
                        window.set_cursor_position(smoke.cursor);
                        smoke.button_since = smoke.frames;
                        smoke.menu_stage = 14;
                    }
                    16 if time.elapsed_secs() - smoke.menu_since >= 0.15 => {
                        let action = controller.last_menu_action.as_ref().unwrap();
                        assert!(!action.checked);
                        assert_eq!(Some(action.item), smoke.menu_item);
                        assert!(!runtime.is_popup_menu_open());
                        smoke.menu_stage = 17;
                        println!(
                            "demo smoke: named menu action delivered after close, disabled row ignored, hidden row folded, caption updated, retained row toggled on reopen"
                        );
                    }
                    _ => {}
                }
                move_pointer(window_entity, window.cursor_position(), &mut pointers);
                return;
            }
            if page == "Text" && smoke.link_stage < 10 {
                let root = runtime
                    .root_object(controller.basics_page.unwrap())
                    .unwrap();
                let text = runtime.store().child_by_name(root, "n12").unwrap();
                let entity = runtime.entity_of(text).unwrap();
                match smoke.link_stage {
                    0 => {
                        let (computed, transform) = nodes.get(entity).unwrap();
                        let (block, layout) = rich_layouts.get(entity).unwrap();
                        let run = layout
                            .run_geometry
                            .iter()
                            .find(|run| {
                                block
                                    .entities()
                                    .get(run.section_index as usize)
                                    .is_some_and(|section| {
                                        text_spans
                                            .get(section.entity)
                                            .is_ok_and(|span| span.0.contains("I am link"))
                                    })
                            })
                            .expect("official link must have measured runs");
                        smoke.link_original = Some(
                            runtime
                                .store()
                                .object(text)
                                .unwrap()
                                .control()
                                .as_text_field()
                                .unwrap()
                                .text
                                .clone(),
                        );
                        let local = run.bounds.center() + computed.content_box().min;
                        smoke.cursor = Some(transform.transform_point2(local) / scale_factor);
                        window.set_cursor_position(smoke.cursor);
                        smoke.inline_since = smoke.frames;
                        smoke.link_stage = 1;
                    }
                    1 if smoke.frames - smoke.inline_since >= 4 => {
                        mouse.press(MouseButton::Left);
                        smoke.link_stage = 2;
                    }
                    2 => {
                        mouse.release(MouseButton::Left);
                        smoke.inline_since = smoke.frames;
                        smoke.link_stage = 3;
                    }
                    3 if smoke.frames - smoke.inline_since >= 12 => {
                        assert_eq!(smoke.link_messages, 1);
                        let value = &runtime
                            .store()
                            .object(text)
                            .unwrap()
                            .control()
                            .as_text_field()
                            .unwrap()
                            .text;
                        assert!(value.ends_with("你点击了链接[/color]：event:xx"), "{value}");
                        assert_eq!(
                            inline_parts
                                .get(entity)
                                .map_or(0, |parts| parts.iter().count()),
                            1,
                            "the replacement's package MovieClip renders as one owned inline image"
                        );
                        commands
                            .spawn(Screenshot::primary_window())
                            .observe(save_to_disk("target/fairygui-demo-link-click.png"));
                        smoke.link_stage = 4;
                        smoke.inline_since = smoke.frames;
                    }
                    4 if smoke.frames - smoke.inline_since >= 12 => {
                        runtime.store_mut().set_object_text(
                            text,
                            "[url=event:xx][img]ui://9leh0eyfrpmb6[/img][/url]",
                        );
                        runtime.store_mut().set_scale(text, -1.1, 0.85);
                        runtime.store_mut().set_rotation(text, 7.0);
                        println!(
                            "demo smoke: measured rich-text link click published event:xx once, updated the official text and reclaimed its old image"
                        );
                        smoke.link_stage = 5;
                        smoke.inline_since = smoke.frames;
                    }
                    5 if smoke.frames - smoke.inline_since >= 12 => {
                        let (computed, transform) = nodes.get(entity).unwrap();
                        let (_, layout) = rich_layouts.get(entity).unwrap();
                        assert!(
                            layout.glyphs.is_empty(),
                            "an image-only link has no dummy text glyphs"
                        );
                        assert_eq!(layout.inline_boxes.len(), 1);
                        let local = computed.content_box().min + layout.inline_boxes[0].2.center();
                        smoke.cursor = Some(transform.transform_point2(local) / scale_factor);
                        window.set_cursor_position(smoke.cursor);
                        commands
                            .spawn(Screenshot::primary_window())
                            .observe(save_to_disk("target/fairygui-demo-inline-link.png"));
                        smoke.link_stage = 6;
                        smoke.inline_since = smoke.frames;
                    }
                    6 if smoke.frames - smoke.inline_since >= 4 => {
                        mouse.press(MouseButton::Left);
                        smoke.link_stage = 7;
                    }
                    7 => {
                        mouse.release(MouseButton::Left);
                        smoke.link_stage = 8;
                        smoke.inline_since = smoke.frames;
                    }
                    8 if smoke.frames - smoke.inline_since >= 12 => {
                        assert_eq!(
                            smoke.link_messages, 2,
                            "transformed image link activates once"
                        );
                        let value = &runtime
                            .store()
                            .object(text)
                            .unwrap()
                            .control()
                            .as_text_field()
                            .unwrap()
                            .text;
                        assert!(value.ends_with("你点击了链接[/color]：event:xx"), "{value}");
                        smoke.link_stage = 9;
                        smoke.inline_since = smoke.frames;
                    }
                    9 if smoke.frames - smoke.inline_since >= 12 => {
                        runtime
                            .store_mut()
                            .set_object_text(text, smoke.link_original.as_ref().unwrap());
                        runtime.store_mut().set_scale(text, 1.0, 1.0);
                        runtime.store_mut().set_rotation(text, 0.0);
                        println!(
                            "demo smoke: image-only InlineBox link hit through mirroring/rotation, without placeholder glyphs"
                        );
                        smoke.link_stage = 10;
                    }
                    _ => {}
                }
                move_pointer(window_entity, window.cursor_position(), &mut pointers);
                return;
            }
            if page == "Text" && smoke.inline_stage < 8 {
                let root = runtime
                    .root_object(controller.basics_page.unwrap())
                    .unwrap();
                let text = *smoke
                    .inline_text
                    .get_or_insert_with(|| runtime.store().child_by_name(root, "n12").unwrap());
                let entity = runtime.entity_of(text).unwrap();
                match smoke.inline_stage {
                    0 => {
                        let object = runtime.store().object(text).unwrap();
                        smoke.inline_original = Some((
                            object.control().as_text_field().unwrap().text.clone(),
                            Vec2::new(object.x(), object.y()),
                        ));
                        smoke.inline_images = inline_parts.get(entity).unwrap().iter().collect();
                        assert_eq!(smoke.inline_images.len(), 1);
                        let menu = assets.get(demo.menu_asset.as_ref().unwrap()).unwrap();
                        let external = menu
                            .package
                            .items()
                            .iter()
                            .enumerate()
                            .find(|(index, _)| menu.package.sprite_for(*index).is_some())
                            .unwrap()
                            .1;
                        let source = format!("ui://{}{}", menu.package.id(), external.id);
                        runtime.store_mut().set_object_text(text, &format!("[b]Inline images[/b]\n[img]ui://9leh0eyfrpmb6[/img] local [img]{source}[/img] MainMenu"));
                        runtime.store_mut().set_xy(text, 450.0, 230.0);
                        runtime.store_mut().set_scale(text, 1.1, 0.8);
                        runtime.store_mut().set_rotation(text, 12.0);
                        runtime.store_mut().set_alpha(text, 0.6);
                        smoke.inline_since = smoke.frames;
                        smoke.inline_stage = 1;
                    }
                    1 if smoke.frames - smoke.inline_since >= 12 => {
                        assert!(
                            smoke
                                .inline_images
                                .iter()
                                .all(|id| layout_nodes.get(*id).is_err())
                        );
                        smoke.inline_images = inline_parts.get(entity).unwrap().iter().collect();
                        assert_eq!(smoke.inline_images.len(), 2);
                        let (_, layout) = rich_layouts.get(entity).unwrap();
                        assert_eq!(layout.inline_boxes.len(), 2);
                        for (anchor, _, bounds) in &layout.inline_boxes {
                            let reserved = inline_boxes.get(*anchor).unwrap();
                            assert!(text_spans.get(*anchor).is_err());
                            assert!(
                                (bounds.size() / layout.scale_factor)
                                    .abs_diff_eq(reserved.size, 0.001)
                            );
                        }
                        for &image in &smoke.inline_images {
                            assert_eq!(layout_nodes.get(image).unwrap().display, Display::Flex);
                            assert!(nodes.get(image).unwrap().0.size().min_element() > 0.0);
                            assert!((images.get(image).unwrap().color.alpha() - 0.6).abs() < 0.001);
                            assert_eq!(
                                object_transforms.get(image).unwrap().rotation,
                                object_transforms.get(entity).unwrap().rotation
                            );
                        }
                        commands
                            .spawn(Screenshot::primary_window())
                            .observe(save_to_disk("target/fairygui-demo-inline-transformed.png"));
                        let container = runtime
                            .store_mut()
                            .create(fairygui_core::package::ObjectType::Component);
                        runtime.store_mut().set_size(container, 600.0, 350.0);
                        runtime.store_mut().set_xy(container, 80.0, 70.0);
                        runtime.store_mut().set_parent(container, Some(root));
                        runtime.queue_subtree(container);
                        runtime.store_mut().set_parent(text, Some(container));
                        smoke.inline_container = Some(container);
                        smoke.inline_since = smoke.frames;
                        smoke.inline_stage = 2;
                    }
                    2 if smoke.frames - smoke.inline_since >= 12 => {
                        let parent = runtime.entity_of(smoke.inline_container.unwrap()).unwrap();
                        assert_eq!(parents.get(entity).unwrap().parent(), parent);
                        for &image in &smoke.inline_images {
                            assert_eq!(parents.get(image).unwrap().parent(), parent);
                        }
                        smoke
                            .inline_pool
                            .release(runtime.store_mut(), "smoke-rich-text", text);
                        smoke.inline_since = smoke.frames;
                        smoke.inline_stage = 3;
                    }
                    3 if smoke.frames - smoke.inline_since >= 12 => {
                        for &image in &smoke.inline_images {
                            assert!(parents.get(image).is_err());
                            assert_eq!(*visibility.get(image).unwrap().0, Visibility::Hidden);
                            assert!(!visibility.get(image).unwrap().1.get());
                        }
                        let pooled = smoke
                            .inline_pool
                            .acquire_checked(runtime.store(), "smoke-rich-text")
                            .unwrap();
                        assert_eq!(pooled, text);
                        runtime.store_mut().set_parent(text, Some(root));
                        runtime.store_mut().set_visible(text, true);
                        let position = smoke.inline_original.as_ref().unwrap().1;
                        runtime.store_mut().set_xy(text, position.x, position.y);
                        runtime.store_mut().set_scale(text, 1.0, 1.0);
                        runtime.store_mut().set_rotation(text, 0.0);
                        runtime.store_mut().set_alpha(text, 1.0);
                        runtime
                            .store_mut()
                            .remove(smoke.inline_container.take().unwrap());
                        smoke.inline_since = smoke.frames;
                        smoke.inline_stage = 4;
                    }
                    4 if smoke.frames - smoke.inline_since >= 12 => {
                        assert_eq!(
                            inline_parts.get(entity).unwrap().iter().collect::<Vec<_>>(),
                            smoke.inline_images
                        );
                        for &image in &smoke.inline_images {
                            assert!(visibility.get(image).unwrap().1.get());
                        }
                        runtime.store_mut().set_object_text(
                            text,
                            "[color=#ffcc00]Updated[/color] [img]ui://9leh0eyfrpmb6[/img]",
                        );
                        smoke.inline_since = smoke.frames;
                        smoke.inline_stage = 5;
                    }
                    5 if smoke.frames - smoke.inline_since >= 12 => {
                        assert!(
                            smoke
                                .inline_images
                                .iter()
                                .all(|id| layout_nodes.get(*id).is_err())
                        );
                        let current: Vec<_> = inline_parts.get(entity).unwrap().iter().collect();
                        assert_eq!(current.len(), 1);
                        assert_eq!(layout_nodes.get(current[0]).unwrap().display, Display::Flex);
                        assert!(visibility.get(current[0]).unwrap().1.get());
                        commands
                            .spawn(Screenshot::primary_window())
                            .observe(save_to_disk("target/fairygui-demo-inline-rebuilt.png"));
                        smoke.inline_since = smoke.frames;
                        smoke.inline_stage = 6;
                    }
                    6 if smoke.frames - smoke.inline_since >= 12 => {
                        runtime
                            .store_mut()
                            .set_object_text(text, &smoke.inline_original.as_ref().unwrap().0);
                        smoke.inline_since = smoke.frames;
                        smoke.inline_stage = 7;
                    }
                    7 if smoke.frames - smoke.inline_since >= 12 => {
                        assert_eq!(inline_parts.get(entity).unwrap().iter().count(), 1);
                        println!(
                            "demo smoke: inline images rebuilt, transformed, reparented, pooled/reattached and restored with owned-entity cleanup verified"
                        );
                        smoke.inline_stage = 8;
                    }
                    _ => {}
                }
                move_pointer(window_entity, window.cursor_position(), &mut pointers);
                return;
            }
            if page == "MovieClip" {
                use fairygui_core::object::AnimationPlaySettings;
                let root = runtime
                    .root_object(controller.basics_page.unwrap())
                    .unwrap();
                let movie = runtime.store().child_by_name(root, "n8").unwrap();
                let entity = runtime.entity_of(movie).unwrap();
                let player = movies.get(entity).unwrap();
                assert!(player.frame_count() > 3);
                match smoke.movie_stage {
                    0 => {
                        smoke.movie_object = Some(movie);
                        runtime.store_mut().set_animation_time_scale(movie, 4.0);
                        runtime.store_mut().set_animation_play_settings(
                            movie,
                            AnimationPlaySettings {
                                start: 1,
                                end: 3,
                                times: 2,
                                end_at: 2,
                            },
                        );
                        smoke.movie_stage = 1;
                        return;
                    }
                    1 => {
                        if !player.is_ended() {
                            return;
                        }
                        assert_eq!(player.frame(), 2);
                        assert_eq!(runtime.store().animation_state(movie), Some((true, 2)));
                        assert_eq!(smoke.movie_completions, 1);
                        runtime.store_mut().rewind_animation(movie);
                        smoke.movie_since = smoke.frames;
                        smoke.movie_stage = 2;
                        return;
                    }
                    2 => {
                        if smoke.frames - smoke.movie_since < 12 {
                            return;
                        }
                        assert!(player.is_ended());
                        assert_eq!(
                            player.frame(),
                            0,
                            "rewind alone must not restart an ended movie"
                        );
                        assert_eq!(smoke.movie_completions, 1);
                        runtime.store_mut().set_animation_play_settings(
                            movie,
                            AnimationPlaySettings {
                                start: 1,
                                end: 2,
                                times: 1,
                                end_at: 1,
                            },
                        );
                        smoke.movie_stage = 3;
                        return;
                    }
                    3 => {
                        if !player.is_ended() {
                            return;
                        }
                        assert_eq!(player.frame(), 1);
                        assert_eq!(smoke.movie_completions, 2);
                        let texture = player.current().expect("visible terminal movie frame");
                        let image = children
                            .get(entity)
                            .unwrap()
                            .iter()
                            .find_map(|child| images.get(child).ok())
                            .expect("movie content image");
                        assert_eq!(
                            image.rect,
                            Some(Rect::new(
                                texture.rect.x,
                                texture.rect.y,
                                texture.rect.x + texture.rect.width,
                                texture.rect.y + texture.rect.height
                            ))
                        );
                        smoke.movie_stage = 4;
                        println!(
                            "demo smoke: MovieClip finite ranges, held end frame, two completion messages, rewind and replay verified in Bevy"
                        );
                    }
                    _ => {}
                }
            }
            if page == "Controller" {
                let root = runtime
                    .root_object(controller.basics_page.unwrap())
                    .unwrap();
                if smoke.controller_stage < 7 {
                    // Use actual pointer input on both related-controller Check buttons.
                    match smoke.controller_stage {
                        0 | 3 => {
                            let (name, controller_name) = if smoke.controller_stage == 0 {
                                ("n11", "c1")
                            } else {
                                let component = runtime
                                    .store()
                                    .object(root)
                                    .unwrap()
                                    .control()
                                    .component()
                                    .unwrap();
                                assert_eq!(component.controller("c1").unwrap().selected_index, 1);
                                ("switchBtn", "c2")
                            };
                            let id = runtime.store().child_by_name(root, name).unwrap();
                            let entity = runtime.entity_of(id).unwrap();
                            let (node, transform) = nodes.get(entity).unwrap();
                            assert!(node.size().min_element() > 0.0);
                            smoke.cursor =
                                Some(transform.transform_point2(Vec2::ZERO) / scale_factor);
                            window.set_cursor_position(smoke.cursor);
                            println!("demo smoke: click Controller/{name} for {controller_name}");
                        }
                        1 | 4 => mouse.press(MouseButton::Left),
                        2 | 5 => mouse.release(MouseButton::Left),
                        6 => {
                            let component = runtime
                                .store()
                                .object(root)
                                .unwrap()
                                .control()
                                .component()
                                .unwrap();
                            assert_eq!(component.controller("c2").unwrap().selected_index, 1);
                            let group = runtime.store().child_by_name(root, "n16").unwrap();
                            runtime.store_mut().set_alpha(group, 0.5);
                            smoke.since = smoke.frames;
                        }
                        _ => unreachable!(),
                    }
                    smoke.controller_stage += 1;
                    move_pointer(window_entity, window.cursor_position(), &mut pointers);
                    return;
                }
                if smoke.frames - smoke.since < 30 || runtime.store().has_gear_tweens() {
                    return;
                }
                let member = runtime.store().child_by_name(root, "n13").unwrap();
                let entity = runtime.entity_of(member).unwrap();
                let (_, inherited) = visibility.get(entity).unwrap();
                assert!(
                    inherited.get(),
                    "group member must be visible in Bevy's hierarchy"
                );
                let (computed, transform) = nodes.get(entity).unwrap();
                assert!(computed.size().min_element() > 0.0);
                println!(
                    "demo smoke: group member center {:?}, size {:?}",
                    transform.transform_point2(Vec2::ZERO) / scale_factor,
                    computed.size() / scale_factor
                );
                assert!(
                    (runtime.store().object(member).unwrap().x() - 890.0).abs() < 0.001,
                    "group tween must move its members"
                );
                assert!(
                    (images.get(entity).unwrap().color.alpha() - 0.5).abs() < 0.001,
                    "group alpha must reach the rendered member"
                );
                let movie = runtime.store().child_by_name(root, "n10").unwrap();
                assert_eq!(
                    runtime.store().animation_state(movie),
                    Some((false, 0)),
                    "animation gear must pause and seek"
                );
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk("target/fairygui-demo-controller.png"));
                println!(
                    "demo smoke: controller button clicks updated member geometry and movie playback; group alpha API updated opacity"
                );
            }
            let id = runtime
                .store()
                .find_by_name(controller.root.unwrap(), "btn_Back")
                .unwrap();
            let entity = runtime.entity_of(id).unwrap();
            let (_, transform) = nodes.get(entity).unwrap();
            smoke.cursor = Some(transform.transform_point2(Vec2::ZERO) / scale_factor);
            window.set_cursor_position(smoke.cursor);
            println!(
                "demo smoke: Basics/{} opened",
                demos::BASICS_PAGES[smoke.basics_entry]
            );
            if demos::BASICS_PAGES[smoke.basics_entry] == "Text" {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk("target/fairygui-demo-text.png"));
            }
            if demos::BASICS_PAGES[smoke.basics_entry] == "Slider" {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk("target/fairygui-demo-slider.png"));
            }
            if page == "MovieClip" {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk("target/fairygui-demo-movieclip.png"));
            }
            if page == "Loader" {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk("target/fairygui-demo-loader.png"));
            }
            smoke.step = 10;
        }
        10 => {
            mouse.press(MouseButton::Left);
            smoke.step = 11;
        }
        11 => {
            mouse.release(MouseButton::Left);
            smoke.step = 12;
            smoke.since = smoke.frames;
        }
        12 if controller.basics_page.is_none() => {
            smoke.basics_entry += 1;
            smoke.step = 3;
        }
        0 if demo.menu.is_some() && demo.open.is_none() && smoke.frames - smoke.since > 60 => {
            let root = runtime.root_object(demo.menu.unwrap()).unwrap();
            let Some(id) = runtime.store().find_by_name(root, button) else {
                return;
            };
            let Some(entity) = runtime.entity_of(id) else {
                return;
            };
            let Ok((node, transform)) = nodes.get(entity) else {
                return;
            };
            if node.size().min_element() <= 0.0 {
                return;
            }
            let point = transform.transform_point2(Vec2::ZERO) / window.scale_factor();
            smoke.cursor = Some(point);
            window.set_cursor_position(smoke.cursor);
            println!("demo smoke: click {package} at {point:?}");
            if smoke.entry == 0 {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk("target/fairygui-demo-menu.png"));
            }
            smoke.step = 1;
            smoke.since = smoke.frames;
        }
        1 if smoke.frames - smoke.since > 15 => {
            if smoke.entry == 0 {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk("target/fairygui-demo-hover.png"));
            }
            mouse.press(MouseButton::Left);
            smoke.step = 2;
        }
        2 => {
            mouse.release(MouseButton::Left);
            smoke.step = 3;
            smoke.since = smoke.frames;
        }
        3 if package == "Transition"
            && smoke.visits == 0
            && smoke.transition_entry < 3
            && demo.open.is_some()
            && smoke.frames - smoke.since > 30 =>
        {
            match smoke.transition_stage {
                0 => {
                    let root = controller.root.unwrap();
                    let name = ["btn3", "btn4", "btn5"][smoke.transition_entry];
                    let id = runtime.store().child_by_name(root, name).unwrap();
                    let (_, transform) = nodes.get(runtime.entity_of(id).unwrap()).unwrap();
                    smoke.cursor = Some(transform.transform_point2(Vec2::ZERO) / scale_factor);
                    window.set_cursor_position(smoke.cursor);
                    smoke.transition_stage = 1;
                }
                1 => {
                    mouse.press(MouseButton::Left);
                    smoke.transition_stage = 2;
                }
                2 => {
                    mouse.release(MouseButton::Left);
                    smoke.transition_stage = 3;
                }
                3 => {
                    if let Some(index) = controller.transition_sample {
                        let id = runtime.root_object(index).unwrap();
                        let (_, time) = runtime.store().transition_state(id, "t0").unwrap();
                        let capture_time = [0.3, 0.6, 0.45][smoke.transition_entry];
                        let nested_transition_ready = smoke.transition_entry != 2
                            || runtime
                                .store()
                                .transition_state(id, "t1")
                                .is_some_and(|(playing, _)| playing);
                        if time >= capture_time && nested_transition_ready {
                            let entity = runtime.entity_of(id).unwrap();
                            assert!(nodes.get(entity).unwrap().0.size().min_element() > 0.0);
                            assert!(visibility.get(entity).unwrap().1.get());
                            if smoke.transition_entry == 1 {
                                let value = runtime
                                    .store()
                                    .find_by_name(id, "value")
                                    .expect("PowerUp sample value label");
                                let text = &runtime
                                    .store()
                                    .object(value)
                                    .unwrap()
                                    .control()
                                    .as_text_field()
                                    .unwrap()
                                    .text;
                                assert!(
                                    text.parse::<i32>().unwrap() > 10000,
                                    "PowerUp hook must advance its counter"
                                );
                                assert_eq!(
                                    glyphs
                                        .get(runtime.entity_of(value).unwrap())
                                        .unwrap()
                                        .0
                                        .len(),
                                    5
                                );
                            }
                            if smoke.transition_entry == 2 {
                                assert!(runtime.store().transition_state(id, "t1").unwrap().0);
                                let target = runtime.store().child_by_name(id, "n2").unwrap();
                                let actual = object_transforms
                                    .get(runtime.entity_of(target).unwrap())
                                    .unwrap()
                                    .scale
                                    .x;
                                assert!(
                                    (actual - runtime.store().object(target).unwrap().scale_x())
                                        .abs()
                                        < 0.001
                                );
                                println!(
                                    "demo smoke: PathDemo nested t1 is playing and Bevy reflects its scale"
                                );
                            }
                            let name = ["goodhit", "powerup", "path"][smoke.transition_entry];
                            commands
                                .spawn(Screenshot::primary_window())
                                .observe(save_to_disk(format!(
                                    "target/fairygui-demo-transition-{name}.png"
                                )));
                            println!("demo smoke: Transition/{name} playing through button click");
                            smoke.transition_stage = match smoke.transition_entry {
                                1 => 10,
                                2 => 5,
                                _ => 4,
                            };
                        }
                    }
                }
                4 if controller.transition_sample.is_none() => {
                    let group = runtime
                        .store()
                        .child_by_name(controller.root.unwrap(), "g0")
                        .unwrap();
                    assert!(runtime.store().display_visible(group));
                    println!("demo smoke: transition sample completed and buttons restored");
                    smoke.transition_entry += 1;
                    smoke.transition_stage = 0;
                    smoke.since = smoke.frames;
                    if smoke.transition_only && smoke.transition_entry == 3 {
                        println!("demo smoke: transition observer scenarios passed");
                        exit.write(AppExit::Success);
                    }
                }
                5 => {
                    let id = runtime
                        .root_object(controller.transition_sample.unwrap())
                        .unwrap();
                    let target = runtime.store().child_by_name(id, "n2").unwrap();
                    let object = runtime.store().object(target).unwrap();
                    smoke.controls_snapshot = Some((
                        runtime.store().transition_state(id, "t0").unwrap().1,
                        runtime.store().transition_state(id, "t1").unwrap().1,
                        Vec2::new(object.x(), object.y()),
                        object.scale_x(),
                    ));
                    runtime.store_mut().set_transition_paused(id, "t0", true);
                    smoke.controls_since = smoke.frames;
                    smoke.transition_stage = 6;
                }
                6 if smoke.frames - smoke.controls_since >= 12 => {
                    let id = runtime
                        .root_object(controller.transition_sample.unwrap())
                        .unwrap();
                    let (parent_time, child_time, point, scale) = smoke.controls_snapshot.unwrap();
                    assert_eq!(
                        runtime.store().transition_state(id, "t0").unwrap().1,
                        parent_time
                    );
                    assert_eq!(
                        runtime.store().transition_state(id, "t1").unwrap().1,
                        child_time
                    );
                    let target = runtime.store().child_by_name(id, "n2").unwrap();
                    let object = runtime.store().object(target).unwrap();
                    assert_eq!(Vec2::new(object.x(), object.y()), point);
                    assert_eq!(object.scale_x(), scale);
                    runtime.store_mut().set_transition_time_scale(id, "t0", 2.0);
                    runtime.store_mut().set_transition_paused(id, "t0", false);
                    smoke.controls_since = smoke.frames;
                    smoke.transition_stage = 7;
                }
                7 if smoke.frames - smoke.controls_since >= 12 => {
                    let id = runtime
                        .root_object(controller.transition_sample.unwrap())
                        .unwrap();
                    assert!(
                        runtime.store().transition_state(id, "t0").unwrap().1
                            > smoke.controls_snapshot.unwrap().0
                    );
                    assert_eq!(runtime.store().transition_time_scale(id, "t1"), Some(2.0));
                    runtime.store_mut().play_transition_reverse(id, "t0", 0.0);
                    assert_eq!(runtime.store().transition_reversed(id, "t0"), Some(true));
                    smoke.controls_since = smoke.frames;
                    smoke.transition_stage = 8;
                }
                8 if smoke.frames - smoke.controls_since >= 12 => {
                    let id = runtime
                        .root_object(controller.transition_sample.unwrap())
                        .unwrap();
                    assert!(runtime.store().transition_state(id, "t0").unwrap().1 > 0.0);
                    let target = runtime.store().child_by_name(id, "n2").unwrap();
                    let actual = object_transforms
                        .get(runtime.entity_of(target).unwrap())
                        .unwrap()
                        .scale
                        .x;
                    assert!(
                        (actual - runtime.store().object(target).unwrap().scale_x()).abs() < 0.001
                    );
                    commands
                        .spawn(Screenshot::primary_window())
                        .observe(save_to_disk("target/fairygui-demo-transition-reverse.png"));
                    println!(
                        "demo smoke: PathDemo pause/resume, inherited 2x speed and reverse playback verified"
                    );
                    smoke.transition_stage = 9;
                }
                9 => {
                    let id = runtime
                        .root_object(controller.transition_sample.unwrap())
                        .unwrap();
                    runtime.store_mut().stop_transition(id, "t0");
                    assert!(!runtime.store().transition_state(id, "t1").unwrap().0);
                    smoke.transition_stage = 4;
                }
                10 => {
                    use fairygui_core::transition::{
                        ActionType, TransitionPlayOptions, TransitionValue,
                    };
                    let id = runtime
                        .root_object(controller.transition_sample.unwrap())
                        .unwrap();
                    let movie = runtime.store().child_by_name(id, "powEffect").unwrap();
                    let mut expected = movies
                        .get(runtime.entity_of(movie).unwrap())
                        .unwrap()
                        .clone();
                    let cue = runtime
                        .store()
                        .object(id)
                        .unwrap()
                        .control()
                        .component()
                        .unwrap()
                        .transitions[0]
                        .items
                        .iter()
                        .find(|item| item.action == ActionType::Animation)
                        .unwrap();
                    let start_time = cue.time + 0.25;
                    if let TransitionValue::Animation { frame, .. } = cue.value
                        && frame >= 0
                        && frame != expected.frame() as i32
                    {
                        expected.goto(frame);
                    }
                    expected.advance_time(0.25);
                    runtime
                        .store_mut()
                        .stop_transition_with_completion(id, "t0", false);
                    runtime.store_mut().set_xy(id, 450.0, 450.0);
                    runtime.store_mut().play_transition_with_options(
                        id,
                        "t0",
                        TransitionPlayOptions {
                            start_time,
                            end_time: Some(start_time + 0.25),
                            ..Default::default()
                        },
                    );
                    runtime.store_mut().set_transition_paused(id, "t0", true);
                    smoke.interval_snapshot = Some((start_time, expected.frame()));
                    smoke.controls_since = smoke.frames;
                    smoke.transition_stage = 11;
                }
                11 if smoke.frames - smoke.controls_since >= 12 => {
                    let id = runtime
                        .root_object(controller.transition_sample.unwrap())
                        .unwrap();
                    let movie = runtime.store().child_by_name(id, "powEffect").unwrap();
                    let (start, frame) = smoke.interval_snapshot.unwrap();
                    assert_eq!(
                        runtime.store().transition_state(id, "t0"),
                        Some((true, start))
                    );
                    assert_eq!(
                        runtime.store().animation_state(movie),
                        Some((false, frame as i32))
                    );
                    assert_eq!(
                        movies
                            .get(runtime.entity_of(movie).unwrap())
                            .unwrap()
                            .frame(),
                        frame
                    );
                    commands
                        .spawn(Screenshot::primary_window())
                        .observe(save_to_disk("target/fairygui-demo-transition-interval.png"));
                    println!(
                        "demo smoke: PowerUp interval start and unscaled movie catch-up verified at frame {frame}"
                    );
                    smoke.transition_stage = 12;
                }
                12 => {
                    use fairygui_core::transition::{
                        TransitionEditValue as Edit, TransitionPlayOptions,
                    };
                    let id = runtime
                        .root_object(controller.transition_sample.unwrap())
                        .unwrap();
                    let target = runtime.store().find_by_name(id, "value").unwrap();
                    runtime
                        .store_mut()
                        .stop_transition_with_completion(id, "t0", false);
                    runtime.store_mut().set_xy(id, 450.0, 450.0);
                    assert_eq!(
                        runtime
                            .store_mut()
                            .set_transition_value(
                                id,
                                "t0",
                                "play_num_now",
                                Edit::Pair { x: 10.0, y: 20.0 }
                            )
                            .unwrap(),
                        2
                    );
                    runtime
                        .store_mut()
                        .set_transition_value(id, "t0", "end", Edit::Pair { x: 60.0, y: -100.0 })
                        .unwrap();
                    runtime
                        .store_mut()
                        .set_transition_duration(id, "t0", "play_num_now", 0.5)
                        .unwrap();
                    runtime
                        .store_mut()
                        .set_transition_target(id, "t0", "play_num_now", Some(target))
                        .unwrap();
                    runtime
                        .store_mut()
                        .set_transition_hook(id, "t0", "play_num_now", Some(9001))
                        .unwrap();
                    let start = runtime
                        .store()
                        .transition_label_time(id, "t0", "play_num_now")
                        .unwrap()
                        + 0.25;
                    runtime.store_mut().play_transition_with_options(
                        id,
                        "t0",
                        TransitionPlayOptions {
                            start_time: start,
                            ..Default::default()
                        },
                    );
                    runtime.store_mut().set_transition_paused(id, "t0", true);
                    let object = runtime.store().object(target).unwrap();
                    assert!((object.x() - 38.0).abs() < 0.001);
                    assert!((object.y() - 20.0).abs() < 0.001);
                    smoke.controls_since = smoke.frames;
                    smoke.transition_stage = 13;
                }
                13 if smoke.frames - smoke.controls_since >= 12 => {
                    let id = runtime
                        .root_object(controller.transition_sample.unwrap())
                        .unwrap();
                    let target = runtime.store().find_by_name(id, "value").unwrap();
                    let node = layout_nodes
                        .get(runtime.entity_of(target).unwrap())
                        .unwrap();
                    assert!(matches!(node.left, Val::Px(value) if (value - 38.0).abs() < 0.001));
                    assert_eq!(node.top, Val::Px(20.0));
                    assert_eq!(smoke.edit_callbacks, 1);
                    commands
                        .spawn(Screenshot::primary_window())
                        .observe(save_to_disk("target/fairygui-demo-transition-edit.png"));
                    println!(
                        "demo smoke: PowerUp edited values/duration/target, first-match callback and Bevy node position verified"
                    );
                    runtime
                        .store_mut()
                        .clear_transition_hooks(id, "t0")
                        .unwrap();
                    runtime.store_mut().set_transition_paused(id, "t0", false);
                    smoke.transition_stage = 4;
                }
                _ => {}
            }
        }
        3 if package == "Transition"
            && smoke.visits == 0
            && smoke.transition_entry >= 3
            && smoke.lifecycle_stage < 6 =>
        {
            match smoke.lifecycle_stage {
                0 => {
                    let (_, handle, _) = demo.open.as_ref().unwrap();
                    let asset = assets.get(handle).unwrap();
                    let index = runtime
                        .create_root(handle.clone(), &asset.package, "PowerUp")
                        .unwrap();
                    let owner = runtime.root_object(index).unwrap();
                    runtime.store_mut().set_xy(owner, 450.0, 450.0);
                    for (name, text) in [("value", "10000"), ("add_value", "+2000")] {
                        let id = runtime.store().child_by_name(owner, name).unwrap();
                        runtime.store_mut().set_object_text(id, text);
                    }
                    runtime
                        .store_mut()
                        .set_transition_auto_play_with_options(owner, "t0", true, -1, 0.25)
                        .unwrap();
                    smoke.lifecycle_root = Some(index);
                    smoke.lifecycle_stage = 1;
                }
                1 => {
                    let index = smoke.lifecycle_root.unwrap();
                    let owner = runtime.root_object(index).unwrap();
                    if runtime.store().transition_state(owner, "t0").unwrap().1 < 0.25 {
                        return;
                    }
                    smoke.lifecycle_entity = runtime.entity_of(owner);
                    smoke.lifecycle_time = runtime.store().transition_state(owner, "t0").unwrap().1;
                    runtime.set_root_attached(index, false);
                    assert!(!runtime.store().is_on_stage(owner));
                    assert!(runtime.store().object(owner).unwrap().visible());
                    smoke.lifecycle_since = smoke.frames;
                    smoke.lifecycle_stage = 2;
                }
                2 if smoke.frames - smoke.lifecycle_since >= 12 => {
                    let index = smoke.lifecycle_root.unwrap();
                    let owner = runtime.root_object(index).unwrap();
                    assert_eq!(
                        runtime.store().transition_state(owner, "t0"),
                        Some((false, smoke.lifecycle_time))
                    );
                    let (visible, inherited) =
                        visibility.get(smoke.lifecycle_entity.unwrap()).unwrap();
                    assert_eq!(*visible, Visibility::Hidden);
                    assert!(!inherited.get());
                    runtime.set_root_attached(index, true);
                    assert_eq!(runtime.entity_of(owner), smoke.lifecycle_entity);
                    assert_eq!(
                        runtime.store().transition_state(owner, "t0"),
                        Some((true, 0.0))
                    );
                    smoke.lifecycle_stage = 3;
                }
                3 => {
                    let index = smoke.lifecycle_root.unwrap();
                    let owner = runtime.root_object(index).unwrap();
                    let (_, time) = runtime.store().transition_state(owner, "t0").unwrap();
                    if time < 0.25 {
                        return;
                    }
                    assert!(
                        visibility
                            .get(smoke.lifecycle_entity.unwrap())
                            .unwrap()
                            .1
                            .get()
                    );
                    runtime
                        .store_mut()
                        .set_transition_auto_play(owner, "t0", false)
                        .unwrap();
                    assert!(runtime.store().transition_state(owner, "t0").unwrap().0);
                    smoke.lifecycle_time = time;
                    smoke.lifecycle_since = smoke.frames;
                    smoke.lifecycle_stage = 4;
                }
                4 if smoke.frames - smoke.lifecycle_since >= 12 => {
                    let index = smoke.lifecycle_root.unwrap();
                    let owner = runtime.root_object(index).unwrap();
                    assert!(
                        runtime.store().transition_state(owner, "t0").unwrap().1
                            > smoke.lifecycle_time
                    );
                    commands
                        .spawn(Screenshot::primary_window())
                        .observe(save_to_disk("target/fairygui-demo-stage.png"));
                    smoke.lifecycle_since = smoke.frames;
                    smoke.lifecycle_stage = 5;
                }
                5 if smoke.frames - smoke.lifecycle_since >= 12 => {
                    let index = smoke.lifecycle_root.unwrap();
                    runtime.remove_root(&mut commands, index);
                    assert_eq!(runtime.root_object(index), None);
                    smoke.lifecycle_stage = 6;
                    println!(
                        "demo smoke: auto-play delay, detach stop, hidden retained view, reentry on same entity, on-stage disable and disposal verified"
                    );
                }
                _ => {}
            }
        }
        3 if package == "Cooldown"
            && demo.open.is_some()
            && !smoke.fills.complete()
            && smoke.frames - smoke.since > 30 =>
        {
            let frames = smoke.frames;
            let root = controller.root.unwrap();
            fills::drive(
                &mut smoke.fills,
                frames,
                &mut runtime,
                &mut commands,
                &nodes,
                root,
            );
        }
        3 if demo
            .open
            .as_ref()
            .is_some_and(|(_, _, name)| name == package)
            && smoke.frames - smoke.since > 60
            && !global_wait.is_requested() =>
        {
            let id = demo.close_button.unwrap();
            let Some(entity) = runtime.entity_of(id) else {
                return;
            };
            let Ok((node, transform)) = nodes.get(entity) else {
                return;
            };
            if node.size().min_element() <= 0.0 {
                return;
            }
            if smoke.entry == 0 {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk("target/fairygui-demo-basics.png"));
            }
            if package == "Basics" && smoke.basics_entry == 5 {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk("target/fairygui-demo-list.png"));
            }
            if package == "Basics" && smoke.basics_entry == 6 {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk("target/fairygui-demo-grid.png"));
            }
            let point = transform.transform_point2(Vec2::ZERO) / window.scale_factor();
            smoke.cursor = Some(point);
            window.set_cursor_position(smoke.cursor);
            println!("demo smoke: {package} opened; close at {point:?}");
            smoke.step = 4;
        }
        4 => {
            mouse.press(MouseButton::Left);
            smoke.step = 5;
        }
        5 => {
            mouse.release(MouseButton::Left);
            smoke.step = 6;
            smoke.since = smoke.frames;
        }
        6 if demo.open.is_none() => {
            smoke.visits += 1;
            if smoke.visits == 2 {
                smoke.entry += 1;
                smoke.visits = 0;
            }
            if smoke.entry == catalog::ENTRIES.len() {
                println!(
                    "demo smoke PASSED: all {} demos opened twice and returned to menu",
                    smoke.entry,
                );
                exit.write(AppExit::Success);
                // Prevent indexing past the catalog if exit takes another frame.
                smoke.entry = 0;
            }
            smoke.step = 0;
        }
        _ => {}
    }
    // Picking uses pointer messages; setting Window's cursor alone only updates
    // the legacy UI focus system and RelativeCursorPosition.
    move_pointer(window_entity, window.cursor_position(), &mut pointers);
}

fn move_pointer(
    window_entity: Entity,
    position: Option<Vec2>,
    pointers: &mut MessageWriter<PointerInput>,
) {
    if let Some(position) = position {
        let target =
            bevy::camera::RenderTarget::Window(bevy::window::WindowRef::Entity(window_entity))
                .normalize(Some(window_entity))
                .unwrap();
        pointers.write(PointerInput::new(
            PointerId::Mouse,
            Location { target, position },
            PointerAction::Move { delta: Vec2::ZERO },
        ));
    }
}
