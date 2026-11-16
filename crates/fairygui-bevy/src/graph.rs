//! Graph drawing surfaces use Bevy's normal UI clipping and ordering.
use crate::{GObjectRef, GuiRuntime, backend};
use bevy::{
    asset::embedded_asset,
    prelude::*,
    render::{
        RenderApp,
        render_resource::{AsBindGroup, ShaderType},
        storage::ShaderBuffer,
    },
    shader::ShaderRef,
    ui_render::{UiMaterialPlugin, ui_material::UiMaterial},
};
use fairygui_core::{
    GObjectId, Size,
    graph::{polygon_draw_offset, polygon_points, polygon_stroke},
    widget::{ControlData, GraphData, GraphType},
};

#[derive(Clone, Copy, Debug, PartialEq, ShaderType)]
struct PolygonUniform {
    canvas: Vec4,
    fill: Vec4,
    stroke: Vec4,
    counts: UVec4,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(crate) struct PolygonMaterial {
    #[uniform(0)]
    uniform: PolygonUniform,
    #[storage(1, read_only)]
    geometry: Handle<ShaderBuffer>,
}
impl UiMaterial for PolygonMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://fairygui_bevy/polygon.wesl".into()
    }
}

pub(crate) fn install(app: &mut App) {
    if app.get_sub_app(RenderApp).is_some() {
        embedded_asset!(app, "polygon.wesl");
        app.add_plugins(UiMaterialPlugin::<PolygonMaterial>::default());
    } else {
        app.init_asset::<PolygonMaterial>();
    }
    if !app.world().contains_resource::<Assets<ShaderBuffer>>() {
        app.init_asset::<ShaderBuffer>();
    }
}

#[derive(Component, Debug)]
pub(crate) struct GraphSurface {
    pub canvas: Vec4,
}

#[derive(Component, Debug)]
pub(crate) struct GraphRender {
    data: GraphData,
    size: Size,
    surface: Option<Entity>,
    material: Option<Handle<PolygonMaterial>>,
}

fn colors(gui: &GuiRuntime, id: GObjectId, data: &GraphData) -> (Vec4, Vec4) {
    let object = gui.store().object(id).unwrap();
    let alpha = gui.paint_alpha(id);
    let color = |source| {
        let c = backend::apply_alpha(gui.paint_tint(object, Some(source)), alpha).to_linear();
        Vec4::new(c.red, c.green, c.blue, c.alpha)
    };
    (color(data.drawn_fill_color()), color(data.line_color))
}

fn geometry(data: &GraphData, size: Size) -> Option<(Vec<Vec4>, Vec4, UVec4)> {
    if data.graph_kind() == GraphType::Rect && data.corner_radius.is_none() {
        return rectangle_geometry(data, size);
    }
    let points = polygon_points(data, size);
    if points.len() < 2 {
        return None;
    }
    let offset = polygon_draw_offset(data);
    let mut vertices: Vec<_> = points
        .iter()
        .map(|p| Vec4::new(p.x as f32 + offset, p.y as f32 + offset, 0.0, 0.0))
        .collect();
    let count = vertices.len();
    if data.has_stroke() {
        let mesh = polygon_stroke(&points, data.line_size as f32);
        vertices.extend(mesh.indices.iter().map(|&i| {
            let p = mesh.vertices[i];
            Vec4::new(p.x + offset, p.y + offset, 0.0, 0.0)
        }));
    }
    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    for p in &vertices {
        min = min.min(p.truncate().truncate());
        max = max.max(p.truncate().truncate());
    }
    // The draw surface includes stroke miters and vertices outside object bounds.
    min -= Vec2::ONE;
    max += Vec2::ONE;
    let extent = max - min;
    if !extent.is_finite() || extent.min_element() <= 0.0 {
        return None;
    }
    let counts = UVec4::new(count as u32, ((vertices.len() - count) / 3) as u32, 0, 0);
    Some((
        vertices,
        Vec4::new(min.x, min.y, extent.x, extent.y),
        counts,
    ))
}

fn rectangle_geometry(data: &GraphData, size: Size) -> Option<(Vec<Vec4>, Vec4, UVec4)> {
    let drawing = fairygui_core::graph::rectangle_drawing(data, size)?;
    let quad = |r: fairygui_core::Rect| Vec4::new(r.x, r.y, r.width, r.height);
    // Slot zero is always the fill. Keep native stroke order and signed sizes.
    let mut vertices = vec![quad(drawing.fill.unwrap_or(fairygui_core::Rect::ZERO))];
    if let Some(stroke) = drawing.stroke {
        vertices.extend(stroke.into_iter().map(quad));
    }
    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    for (index, q) in vertices.iter().enumerate() {
        if (index == 0 && !data.has_fill())
            || (index > 0 && !data.has_stroke())
            || q.z == 0.
            || q.w == 0.
        {
            continue;
        }
        let p = Vec2::new(q.x, q.y);
        let end = p + Vec2::new(q.z, q.w);
        min = min.min(p.min(end));
        max = max.max(p.max(end));
    }
    min -= Vec2::ONE;
    max += Vec2::ONE;
    let extent = max - min;
    if !extent.is_finite() || extent.min_element() <= 0. {
        return None;
    }
    let counts = UVec4::new(vertices.len() as u32, 0, 1, 0);
    Some((
        vertices,
        Vec4::new(min.x, min.y, extent.x, extent.y),
        counts,
    ))
}

fn surface_node(canvas: Vec4) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(canvas.x),
        top: Val::Px(canvas.y),
        width: Val::Px(canvas.z),
        height: Val::Px(canvas.w),
        ..default()
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn sync_graphs(
    mut commands: Commands,
    gui: NonSend<GuiRuntime>,
    materials: Option<ResMut<Assets<PolygonMaterial>>>,
    buffers: Option<ResMut<Assets<ShaderBuffer>>>,
    roots: Query<(Entity, &GObjectRef, Option<&GraphRender>), Without<GraphSurface>>,
    mut basic_surfaces: Query<(&mut BackgroundColor, &mut BorderColor), With<GraphSurface>>,
) {
    let (Some(mut materials), Some(mut buffers)) = (materials, buffers) else {
        return;
    };
    for (entity, object, previous) in &roots {
        let Some(object_data) = gui.store().object(object.0) else {
            continue;
        };
        let ControlData::Graph(data) = object_data.control() else {
            continue;
        };
        let size = object_data.size();
        let plain_rectangle = data.graph_kind() == GraphType::Rect && data.corner_radius.is_none();
        let (fill, stroke) = colors(&gui, object.0, data);
        if previous.is_some_and(|old| old.data == *data && old.size == size) {
            if let Some(mut material) = previous
                .and_then(|old| old.material.as_ref())
                .and_then(|handle| materials.get_mut(handle))
                && (material.uniform.fill != fill || material.uniform.stroke != stroke)
            {
                material.uniform.fill = fill;
                material.uniform.stroke = stroke;
            }
            if data.graph_kind().is_basic_shape()
                && !plain_rectangle
                && let Some(surface) = previous.and_then(|old| old.surface)
                && let Ok((mut background, mut border)) = basic_surfaces.get_mut(surface)
            {
                let fill = Color::linear_rgba(fill.x, fill.y, fill.z, fill.w);
                let stroke =
                    BorderColor::all(Color::linear_rgba(stroke.x, stroke.y, stroke.z, stroke.w));
                if background.0 != fill {
                    background.0 = fill;
                }
                if *border != stroke {
                    *border = stroke;
                }
            }
            continue;
        }
        if data.graph_kind().is_basic_shape() && !plain_rectangle {
            commands
                .entity(entity)
                .remove::<(BackgroundColor, BorderColor)>();
            commands
                .entity(entity)
                .entry::<Node>()
                .and_modify(|mut node| {
                    node.border = UiRect::ZERO;
                    node.border_radius = BorderRadius::ZERO;
                });
            let bounds = if data.graph_kind() == GraphType::Ellipse {
                fairygui_core::graph::circle_bounds(size)
            } else {
                (size.width > 0. && size.height > 0.).then_some(fairygui_core::geom::Rect::new(
                    0.,
                    0.,
                    size.width,
                    size.height,
                ))
            };
            let old_basic = previous
                .filter(|old| old.material.is_none())
                .and_then(|old| old.surface);
            if let Some(old) = previous
                .filter(|old| old.material.is_some())
                .and_then(|old| old.surface)
            {
                commands.entity(old).despawn();
            }
            let surface = bounds.map(|bounds| {
                let style = backend::graph_style(data);
                // Taffy cannot make a border box narrower than its two borders.
                // Keep that paint overflow centered on the declared rectangle;
                // it must never enlarge or shift the logical view itself.
                let width = bounds.width.max(style.stroke_width * 2.);
                let height = bounds.height.max(style.stroke_width * 2.);
                let canvas = Vec4::new(
                    bounds.x + (bounds.width - width) * 0.5,
                    bounds.y + (bounds.height - height) * 0.5,
                    width,
                    height,
                );
                let mut node = surface_node(canvas);
                node.border = UiRect::all(Val::Px(style.stroke_width));
                node.border_radius = style.radius;
                let surface = old_basic.unwrap_or_else(|| {
                    commands
                        .spawn((
                            GObjectRef(object.0),
                            ChildOf(entity),
                            ZIndex(-1),
                            Name::new("FairyGUI shape surface"),
                            bevy::picking::hover::Hovered::default(),
                            bevy::picking::Pickable {
                                should_block_lower: false,
                                is_hoverable: true,
                            },
                            bevy::ui::RelativeCursorPosition::default(),
                        ))
                        .id()
                });
                commands.entity(surface).insert((
                    node,
                    GraphSurface { canvas },
                    BackgroundColor(Color::linear_rgba(fill.x, fill.y, fill.z, fill.w)),
                    BorderColor::all(Color::linear_rgba(stroke.x, stroke.y, stroke.z, stroke.w)),
                ));
                surface
            });
            if bounds.is_none()
                && let Some(old) = old_basic
            {
                commands.entity(old).despawn();
            }
            commands.entity(entity).insert(GraphRender {
                data: data.clone(),
                size,
                surface,
                material: None,
            });
            continue;
        }
        if !plain_rectangle
            && !matches!(
                data.graph_kind(),
                GraphType::Polygon | GraphType::RegularPolygon
            )
        {
            if let Some(surface) = previous.and_then(|old| old.surface) {
                commands.entity(surface).despawn();
            }
            commands.entity(entity).insert(BackgroundColor(Color::NONE));
            commands.entity(entity).remove::<BorderColor>();
            commands
                .entity(entity)
                .entry::<Node>()
                .and_modify(move |mut node| {
                    node.border = UiRect::ZERO;
                    node.border_radius = BorderRadius::ZERO;
                });
            commands.entity(entity).insert(GraphRender {
                data: data.clone(),
                size,
                surface: None,
                material: None,
            });
            continue;
        }
        commands
            .entity(entity)
            .remove::<(BackgroundColor, BorderColor)>();
        commands
            .entity(entity)
            .entry::<Node>()
            .and_modify(|mut node| {
                node.border = UiRect::ZERO;
                node.border_radius = BorderRadius::ZERO;
            });
        let Some((vertices, canvas, counts)) = geometry(data, size) else {
            if let Some(surface) = previous.and_then(|old| old.surface) {
                commands.entity(surface).despawn();
            }
            commands.entity(entity).insert(GraphRender {
                data: data.clone(),
                size,
                surface: None,
                material: None,
            });
            continue;
        };
        let uniform = PolygonUniform {
            canvas,
            fill,
            stroke,
            counts,
        };
        let (surface, handle) = if let Some(old) =
            previous.filter(|old| old.surface.is_some() && old.material.is_some())
        {
            let handle = old.material.as_ref().unwrap().clone();
            let mut material = materials.get_mut(&handle).unwrap();
            *buffers.get_mut(&material.geometry).unwrap() = ShaderBuffer::new(vertices, default());
            material.uniform = uniform;
            let surface = old.surface.unwrap();
            commands
                .entity(surface)
                .insert((surface_node(canvas), GraphSurface { canvas }));
            (surface, handle)
        } else {
            if let Some(old) = previous.and_then(|old| old.surface) {
                commands.entity(old).despawn();
            }
            let buffer = buffers.add(ShaderBuffer::new(vertices, default()));
            let handle = materials.add(PolygonMaterial {
                uniform,
                geometry: buffer,
            });
            let surface = commands
                .spawn((
                    surface_node(canvas),
                    MaterialNode(handle.clone()),
                    GraphSurface { canvas },
                    GObjectRef(object.0),
                    ChildOf(entity),
                    Name::new("FairyGUI primitive surface"),
                    ZIndex(-1),
                    bevy::picking::hover::Hovered::default(),
                    bevy::picking::Pickable {
                        should_block_lower: false,
                        is_hoverable: true,
                    },
                    bevy::ui::RelativeCursorPosition::default(),
                ))
                .id();
            (surface, handle)
        };
        commands.entity(entity).insert(GraphRender {
            data: data.clone(),
            size,
            surface: Some(surface),
            material: Some(handle),
        });
    }
}

#[cfg(test)]
#[path = "graph_tests.rs"]
mod tests;
