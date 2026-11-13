use super::*;

#[derive(Component, Debug)]
pub(crate) struct TextTintSource {
    pub owner: GObjectId,
    pub color: Option<fairygui_core::color::Color>,
}

#[derive(Component, Debug)]
pub(crate) struct TextDecoration;

pub(crate) fn base_color(
    object: &fairygui_core::GObject,
    override_color: Option<fairygui_core::color::Color>,
) -> fairygui_core::color::Color {
    use fairygui_core::color::Color;
    let mut color = override_color.unwrap_or_else(|| match object.control() {
        ControlData::Image(data) => data.color.unwrap_or(Color::WHITE),
        ControlData::MovieClip(data) => data.color.unwrap_or(Color::WHITE),
        ControlData::Loader(data) => data.color.unwrap_or(Color::WHITE),
        ControlData::Text(data) => data.color,
        ControlData::TextInput(data) => data.text_field.color,
        ControlData::Graph(data) => data.fill_color,
        _ => Color::WHITE,
    });
    // GTextField changes its default foreground before drawing glyphs. Explicit
    // span colors and untinted glyph/inline-image white overrides retain theirs.
    if object.grayed()
        && override_color.is_none()
        && matches!(
            object.control(),
            ControlData::Text(_) | ControlData::TextInput(_)
        )
    {
        color.r = 2.0 / 3.0;
        color.g = 2.0 / 3.0;
        color.b = 2.0 / 3.0;
    }
    color
}

/// Legacy FairyGUI-Laya ToolSet policy: gray overrides, then restores the
/// stored local filter. Container offscreen effects are not flattened here.
pub(crate) fn local_color_matrix(
    object: &fairygui_core::GObject,
) -> Option<fairygui_core::color_matrix::ColorMatrix> {
    if object.grayed() {
        Some(fairygui_core::color_matrix::ColorMatrix::LAYA_GRAY)
    } else {
        object
            .color_filter()
            .filter(|f| !f.is_noop())
            .map(|f| f.to_matrix())
    }
}

pub(crate) fn tint(
    object: &fairygui_core::GObject,
    override_color: Option<fairygui_core::color::Color>,
) -> bevy::color::Color {
    let mut color = base_color(object, override_color);
    if let Some(matrix) = local_color_matrix(object) {
        color = matrix.apply(color);
    }
    backend::to_bevy_color(color)
}

/// Recompute appearance from current model values, never from last frame's tint.
/// This also updates rich spans and bitmap glyphs without rebuilding their layout.
#[allow(clippy::type_complexity)]
pub(crate) fn sync_appearance(
    runtime: NonSend<GuiRuntime>,
    mut query: Query<
        (
            Option<&GObjectRef>,
            Option<&TextTintSource>,
            Option<&mut ImageNode>,
            Option<&mut TextColor>,
            Option<&mut BackgroundColor>,
            Option<&mut BorderColor>,
            Option<&TextDecoration>,
        ),
        (
            Or<(With<GObjectRef>, With<TextTintSource>)>,
            Without<crate::graph::GraphSurface>,
            Without<MaterialNode<crate::image_fill::FillMaterial>>,
        ),
    >,
) {
    for (object, source, image, text, background, border, decoration) in &mut query {
        let Some(owner) = object
            .map(|object| object.0)
            .or_else(|| source.map(|source| source.owner))
        else {
            continue;
        };
        let Some(object) = runtime.store().object(owner) else {
            continue;
        };
        let color = backend::apply_alpha(
            runtime.paint_tint(object, source.and_then(|source| source.color)),
            runtime.paint_alpha(owner),
        );
        if let Some(mut image) = image
            && image.color != color
        {
            image.color = color;
        }
        if let Some(mut text) = text
            && text.0 != color
        {
            text.0 = color;
        }
        if let Some(mut background) = background
            && (decoration.is_some() || matches!(object.control(), ControlData::Graph(_)))
        {
            // Circles/polygons own a separate drawing surface; Empty has none.
            let fill = if decoration.is_some()
                || matches!(object.control(), ControlData::Graph(data) if data.graph_kind() == fairygui_core::widget::GraphType::Rect)
            {
                color
            } else {
                Color::NONE
            };
            if background.0 != fill {
                background.0 = fill;
            }
        }
        if let Some(mut border) = border
            && let ControlData::Graph(data) = object.control()
        {
            let stroke = BorderColor::all(backend::apply_alpha(
                runtime.paint_tint(object, Some(data.line_color)),
                runtime.paint_alpha(owner),
            ));
            if *border != stroke {
                *border = stroke;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fairygui_core::color::Color as GuiColor;
    use fairygui_core::package::ObjectType;

    #[test]
    fn solid_graph_colors_use_channel_mixing_and_grayed_text_restores_its_color() {
        use fairygui_core::color_matrix::{ColorFilterParams, ColorMatrix};
        let mut gui = GuiRuntime::default();
        let graph = gui.store_mut().create(ObjectType::Graph);
        let red = GuiColor::new(1.0, 0.0, 0.0, 0.5);
        gui.store_mut().set_control_color(graph, red, None);
        let filter = ColorFilterParams {
            saturation: -1.0,
            ..Default::default()
        };
        gui.store_mut().set_color_filter(graph, Some(filter));
        assert_eq!(
            tint(gui.store().object(graph).unwrap(), None),
            backend::to_bevy_color(filter.to_matrix().apply(red))
        );
        gui.store_mut().set_grayed(graph, true);
        assert_eq!(
            tint(gui.store().object(graph).unwrap(), None),
            backend::to_bevy_color(ColorMatrix::LAYA_GRAY.apply(red))
        );
        let text = gui.store_mut().create(ObjectType::Text);
        gui.store_mut().set_control_color(text, red, None);
        gui.store_mut().set_grayed(text, true);
        let gray = tint(gui.store().object(text).unwrap(), None).to_srgba();
        assert!((gray.red - 2.0 / 3.0).abs() < 1e-6);
        assert!((gray.green - gray.red).abs() < 1e-6 && (gray.blue - gray.red).abs() < 1e-6);
        assert_eq!(gray.alpha, 0.5);
        assert_eq!(
            tint(gui.store().object(text).unwrap(), Some(red)),
            backend::to_bevy_color(ColorMatrix::LAYA_GRAY.apply(red)),
            "explicit rich span colors still pass through gray"
        );
        gui.store_mut().set_grayed(text, false);
        assert_eq!(
            tint(gui.store().object(text).unwrap(), None),
            backend::to_bevy_color(red)
        );
    }

    #[test]
    fn graph_fill_and_border_both_follow_ancestor_and_object_opacity() {
        let package = FuiPackage::parse(
            "Basics",
            include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui"),
        )
        .unwrap();
        let mut gui = GuiRuntime::default();
        let index = gui
            .create_root(Handle::default(), &package, "Demo_Graph")
            .unwrap();
        let root = gui.root_object(index).unwrap();
        let graph = gui.store().child_by_name(root, "n10").unwrap();
        let ControlData::Graph(data) = gui.store().object(graph).unwrap().control() else {
            panic!("graph")
        };
        let (fill, stroke) = (data.fill_color, data.line_color);
        gui.store_mut().set_alpha(root, 0.5);
        gui.store_mut().set_alpha(graph, 0.5);
        let mut app = App::new();
        app.insert_non_send(gui)
            .add_systems(Update, sync_appearance);
        let entity = app
            .world_mut()
            .spawn((
                GObjectRef(graph),
                BackgroundColor::default(),
                BorderColor::all(bevy::color::Color::WHITE),
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<BackgroundColor>(entity).unwrap().0,
            backend::apply_alpha(backend::to_bevy_color(fill), 0.25)
        );
        assert_eq!(
            *app.world().get::<BorderColor>(entity).unwrap(),
            BorderColor::all(backend::apply_alpha(backend::to_bevy_color(stroke), 0.25))
        );
    }

    #[test]
    fn image_movie_loader_graph_and_text_use_current_color_without_accumulating_alpha() {
        let mut app = App::new();
        let mut gui = GuiRuntime::default();
        let parent = gui.store_mut().create(ObjectType::Component);
        gui.store_mut().set_alpha(parent, 0.5);
        let mut ids = Vec::new();
        for kind in [
            ObjectType::Image,
            ObjectType::MovieClip,
            ObjectType::Loader,
            ObjectType::Graph,
            ObjectType::Text,
        ] {
            let id = gui.store_mut().create(kind);
            if kind == ObjectType::Graph {
                gui.store_mut().set_graph(
                    id,
                    crate::core::widget::GraphData {
                        graph_type: 1,
                        ..default()
                    },
                );
            }
            gui.store_mut().set_parent(id, Some(parent));
            gui.store_mut()
                .set_control_color(id, GuiColor::new(1.0, 0.0, 0.0, 1.0), None);
            gui.store_mut().set_alpha(id, 0.5);
            let entity = app
                .world_mut()
                .spawn((
                    GObjectRef(id),
                    ImageNode::default(),
                    TextColor::default(),
                    BackgroundColor::default(),
                ))
                .id();
            ids.push((id, entity));
        }
        app.insert_non_send(gui)
            .add_systems(Update, sync_appearance);
        for _ in 0..30 {
            app.update();
        }
        for &(id, entity) in &ids {
            assert_eq!(
                app.world().get::<ImageNode>(entity).unwrap().color,
                bevy::color::Color::srgba(1.0, 0.0, 0.0, 0.25)
            );
            assert_eq!(
                app.world().get::<TextColor>(entity).unwrap().0,
                bevy::color::Color::srgba(1.0, 0.0, 0.0, 0.25)
            );
            app.world_mut()
                .non_send_mut::<GuiRuntime>()
                .store_mut()
                .set_control_color(id, GuiColor::new(0.0, 0.0, 1.0, 1.0), None);
        }
        app.update();
        for &(_, entity) in &ids {
            assert_eq!(
                app.world().get::<ImageNode>(entity).unwrap().color,
                bevy::color::Color::srgba(0.0, 0.0, 1.0, 0.25)
            );
        }
        assert_eq!(
            app.world().get::<BackgroundColor>(ids[3].1).unwrap().0,
            bevy::color::Color::srgba(0.0, 0.0, 1.0, 0.25)
        );
    }

    #[test]
    fn rich_spans_keep_explicit_colors_while_inherited_spans_and_bitmap_glyphs_follow_the_owner() {
        let mut app = App::new();
        let mut gui = GuiRuntime::default();
        let text = gui.store_mut().create(ObjectType::Text);
        gui.store_mut().set_alpha(text, 0.5);
        let inherited = app
            .world_mut()
            .spawn((
                TextTintSource {
                    owner: text,
                    color: None,
                },
                TextColor::default(),
            ))
            .id();
        let explicit = app
            .world_mut()
            .spawn((
                TextTintSource {
                    owner: text,
                    color: Some(GuiColor::new(0.0, 1.0, 0.0, 1.0)),
                },
                TextColor::default(),
            ))
            .id();
        let glyph = app
            .world_mut()
            .spawn((
                TextTintSource {
                    owner: text,
                    color: None,
                },
                ImageNode::default(),
            ))
            .id();
        app.insert_non_send(gui)
            .add_systems(Update, sync_appearance);
        for color in [
            GuiColor::new(1.0, 0.0, 0.0, 1.0),
            GuiColor::new(0.0, 0.0, 1.0, 1.0),
        ] {
            app.world_mut()
                .non_send_mut::<GuiRuntime>()
                .store_mut()
                .set_control_color(text, color, None);
            app.update();
            let expected = backend::apply_alpha(backend::to_bevy_color(color), 0.5);
            assert_eq!(app.world().get::<TextColor>(inherited).unwrap().0, expected);
            assert_eq!(app.world().get::<ImageNode>(glyph).unwrap().color, expected);
            assert_eq!(
                app.world().get::<TextColor>(explicit).unwrap().0,
                bevy::color::Color::srgba(0.0, 1.0, 0.0, 0.5)
            );
        }
    }
}
