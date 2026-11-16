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
    graph::{circle_drawing, path_fill, polygon_drawing, polygon_stroke, rounded_rectangle_points},
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
    circle_segments: Option<usize>,
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

fn geometry(data: &GraphData, size: Size, scale: f64) -> Option<(Vec<Vec4>, Vec4, UVec4)> {
    if data.graph_kind() == GraphType::Rect && data.corner_radius.is_none() {
        return rectangle_geometry(data, size);
    }
    let rounded_rectangle = data.graph_kind() == GraphType::Rect && data.corner_radius.is_some();
    let circle = circle_drawing(data, size, scale);
    if circle.is_some_and(|circle| circle.radius == 0.) {
        return None;
    }
    let (points, convex) = if rounded_rectangle {
        (rounded_rectangle_points(data, size), false)
    } else if let Some(circle) = circle {
        (circle.points(), true)
    } else {
        let drawing = polygon_drawing(data, size)?;
        (drawing.points, drawing.convex)
    };
    if points.len() < 2 {
        return None;
    }
    let mut vertices: Vec<_> = points
        .iter()
        .map(|p| Vec4::new(p.x as f32, p.y as f32, 0.0, 0.0))
        .collect();
    let count = vertices.len();
    if data.has_stroke() {
        let mesh = polygon_stroke(&points, data.line_size as f32);
        vertices.extend(mesh.indices.iter().map(|&i| {
            let p = mesh.vertices[i];
            Vec4::new(p.x, p.y, 0.0, 0.0)
        }));
    }
    let stroke_triangles = (vertices.len() - count) / 3;
    if data.has_fill() {
        vertices.extend(path_fill(&points, convex).into_iter().map(|index| {
            let p = points[index];
            Vec4::new(p.x as f32, p.y as f32, 0., 0.)
        }));
    }
    let fill_triangles = (vertices.len() - count) / 3 - stroke_triangles;
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
    let counts = UVec4::new(
        count as u32,
        stroke_triangles as u32,
        2,
        fill_triangles as u32,
    );
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

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn sync_graphs(
    mut commands: Commands,
    gui: NonSend<GuiRuntime>,
    materials: Option<ResMut<Assets<PolygonMaterial>>>,
    buffers: Option<ResMut<Assets<ShaderBuffer>>>,
    roots: Query<(Entity, &GObjectRef, Option<&GraphRender>), Without<GraphSurface>>,
    windows: Query<(Entity, &Window, Has<bevy::window::PrimaryWindow>)>,
    settings: Option<Res<crate::GuiInputSettings>>,
    ui_scale: Option<Res<bevy::ui::UiScale>>,
) {
    let (Some(mut materials), Some(mut buffers)) = (materials, buffers) else {
        return;
    };
    let physical_scale = f64::from(
        windows
            .iter()
            .find(|(entity, _, primary)| {
                settings
                    .as_ref()
                    .and_then(|s| s.window)
                    .map_or(*primary, |target| target == *entity)
            })
            .map_or(1., |(_, window, _)| window.scale_factor())
            * ui_scale.map_or(1., |scale| scale.0),
    );
    for (entity, object, previous) in &roots {
        let Some(object_data) = gui.store().object(object.0) else {
            continue;
        };
        let ControlData::Graph(data) = object_data.control() else {
            continue;
        };
        let size = object_data.size();
        let circle_scale = if data.graph_kind() == GraphType::Ellipse {
            gui.store()
                .coordinate_transform(object.0)
                .map_or(physical_scale, |transform| {
                    let (x, y) = transform.axis_scales();
                    physical_scale * x.max(y)
                })
        } else {
            1.
        };
        let circle_segments =
            circle_drawing(data, size, circle_scale).map(|circle| circle.segments);
        let (fill, stroke) = colors(&gui, object.0, data);
        if previous.is_some_and(|old| {
            old.data == *data && old.size == size && old.circle_segments == circle_segments
        }) {
            if let Some(mut material) = previous
                .and_then(|old| old.material.as_ref())
                .and_then(|handle| materials.get_mut(handle))
                && (material.uniform.fill != fill || material.uniform.stroke != stroke)
            {
                material.uniform.fill = fill;
                material.uniform.stroke = stroke;
            }
            continue;
        }
        if !matches!(
            data.graph_kind(),
            GraphType::Rect | GraphType::Ellipse | GraphType::Polygon | GraphType::RegularPolygon
        ) {
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
                circle_segments,
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
        let Some((vertices, canvas, counts)) = geometry(data, size, circle_scale) else {
            if let Some(surface) = previous.and_then(|old| old.surface) {
                commands.entity(surface).despawn();
            }
            commands.entity(entity).insert(GraphRender {
                data: data.clone(),
                size,
                surface: None,
                material: None,
                circle_segments,
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
            circle_segments,
        });
    }
}

#[cfg(test)]
#[path = "graph_tests.rs"]
mod tests;
