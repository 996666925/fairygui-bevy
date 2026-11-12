//! Reuse Bevy UI geometry/draw functions, isolating only filter subtrees and
//! multiply draws. Batch barriers prevent stock UI batching across isolation.
use super::{Group, PaintPolicy};
use bevy::{
    asset::{embedded_asset, load_embedded_asset},
    core_pipeline::{
        FullscreenShader,
        schedule::{Core2d, Core2dSystems, Core3d, Core3dSystems},
        upscaling::upscaling,
    },
    ecs::schedule::ScheduleCleanupPolicy,
    prelude::*,
    render::{
        Extract, ExtractSchedule, Render, RenderApp, RenderStartup, RenderSystems,
        camera::ExtractedCamera,
        render_phase::{PhaseItemExtraIndex, SortedRenderPhase, ViewSortedRenderPhases},
        render_resource::{binding_types::*, *},
        renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery},
        sync_world::MainEntity,
        view::{ExtractedView, RetainedViewEntity, ViewTarget},
    },
    ui_render::{
        UiCameraView, UiViewTarget,
        render_pass::{TransparentUi, ui_pass},
    },
};
use fairygui_core::{color_matrix::ColorMatrix, object::BlendMode};
use std::{collections::HashMap, ops::Range};

#[cfg(test)]
#[path = "compositor_gpu_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
enum Mode {
    #[default]
    Normal,
    Add,
    Multiply,
    Erase,
    Mask,
}
impl From<BlendMode> for Mode {
    fn from(mode: BlendMode) -> Self {
        match mode {
            BlendMode::Add | BlendMode::Screen => Self::Add,
            BlendMode::Multiply => Self::Multiply,
            BlendMode::Erase => Self::Erase,
            BlendMode::Mask => Self::Mask,
            // Laya's unrecognized Sprite blend names use the default factors.
            _ => Self::Normal,
        }
    }
}
fn blend(mode: Mode, premultiplied: bool) -> BlendState {
    let component = |src_factor, dst_factor| BlendComponent {
        src_factor,
        dst_factor,
        operation: BlendOperation::Add,
    };
    let source = if premultiplied {
        BlendFactor::One
    } else {
        BlendFactor::SrcAlpha
    };
    match mode {
        Mode::Normal => BlendState {
            color: component(source, BlendFactor::OneMinusSrcAlpha),
            alpha: component(BlendFactor::One, BlendFactor::OneMinusSrcAlpha),
        },
        Mode::Add => BlendState {
            color: component(source, BlendFactor::One),
            alpha: component(BlendFactor::One, BlendFactor::One),
        },
        Mode::Multiply => BlendState {
            color: component(BlendFactor::Dst, BlendFactor::OneMinusSrcAlpha),
            alpha: component(BlendFactor::DstAlpha, BlendFactor::OneMinusSrcAlpha),
        },
        Mode::Erase | Mode::Mask => {
            let factor = if mode == Mode::Mask {
                BlendFactor::SrcAlpha
            } else {
                BlendFactor::OneMinusSrcAlpha
            };
            BlendState {
                color: component(BlendFactor::Zero, factor),
                alpha: component(BlendFactor::Zero, factor),
            }
        }
    }
}

#[derive(Default, Resource)]
struct Policies {
    nodes: HashMap<MainEntity, (PaintPolicy, Rect)>,
    active: bool,
}
#[allow(clippy::type_complexity)]
fn extract_policies(
    mut policies: ResMut<Policies>,
    nodes: Extract<
        Query<(
            Entity,
            &PaintPolicy,
            &ComputedNode,
            &UiGlobalTransform,
            &InheritedVisibility,
            Option<&CalculatedClip>,
        )>,
    >,
) {
    policies.nodes.clear();
    policies.active = false;
    for (entity, policy, node, transform, visible, clip) in &nodes {
        if !visible.get() || node.is_empty() {
            continue;
        }
        let half = node.size() * 0.5;
        let points = [
            Vec2::new(-half.x, -half.y),
            Vec2::new(half.x, -half.y),
            half,
            Vec2::new(-half.x, half.y),
        ]
        .map(|p| transform.transform_point2(p));
        let mut bounds = Rect::from_corners(
            points.into_iter().fold(Vec2::INFINITY, Vec2::min),
            points.into_iter().fold(Vec2::NEG_INFINITY, Vec2::max),
        );
        if let Some(clip) = clip {
            if clip.is_fully_clipped() {
                continue;
            }
            for rect in clip.rects().unwrap_or_default() {
                let to_world = rect.world_to_clip_local.inverse();
                let corners = [
                    rect.rect.min,
                    Vec2::new(rect.rect.max.x, rect.rect.min.y),
                    rect.rect.max,
                    Vec2::new(rect.rect.min.x, rect.rect.max.y),
                ]
                .map(|p| to_world.transform_point2(p));
                bounds = bounds.intersect(Rect::from_corners(
                    corners.into_iter().fold(Vec2::INFINITY, Vec2::min),
                    corners.into_iter().fold(Vec2::NEG_INFINITY, Vec2::max),
                ));
            }
        }
        policies.active |= !policy.groups.is_empty() || Mode::from(policy.blend) != Mode::Normal;
        policies
            .nodes
            .insert(entity.into(), (policy.clone(), bounds));
    }
}
fn effects(policies: Res<Policies>) -> bool {
    policies.active
}
fn no_effects(policies: Res<Policies>) -> bool {
    !policies.active
}

fn barriers(policies: Res<Policies>, mut phases: ResMut<ViewSortedRenderPhases<TransparentUi>>) {
    if !policies.active {
        return;
    }
    let normal = PaintPolicy::default();
    for phase in phases.values_mut() {
        let items = std::mem::take(&mut phase.items);
        let mut previous: Option<(
            &PaintPolicy,
            CachedRenderPipelineId,
            bevy::render::render_phase::DrawFunctionId,
        )> = None;
        for (_, item) in items {
            let policy = policies.nodes.get(&item.entity.1).map_or(&normal, |p| &p.0);
            if previous.is_some_and(|(old, pipeline, draw)| {
                old != policy
                    || pipeline != item.pipeline
                    || draw != item.draw_function
                    || Mode::from(policy.blend) == Mode::Multiply
            }) {
                phase.add_transient(TransparentUi {
                    sort_key: item.sort_key,
                    entity: (item.entity.0, Entity::PLACEHOLDER.into()),
                    pipeline: item.pipeline,
                    draw_function: item.draw_function,
                    batch_range: 0..0,
                    extra_index: PhaseItemExtraIndex::None,
                    indexed: item.indexed,
                });
            }
            previous = Some((policy, item.pipeline, item.draw_function));
            phase.add_retained(item);
        }
    }
}

#[derive(Clone, Copy, ShaderType)]
struct CompositeUniform {
    screen: Vec4,
    rows: [Vec4; 4],
    offsets: Vec4,
}
impl CompositeUniform {
    fn new(size: Extent3d, matrix: ColorMatrix, opacity: f32) -> Self {
        Self {
            screen: Vec4::new(size.width as f32, size.height as f32, opacity, 0.),
            rows: std::array::from_fn(|i| Vec4::from_slice(&matrix.values[i * 5..i * 5 + 4])),
            offsets: Vec4::new(
                matrix.values[4],
                matrix.values[9],
                matrix.values[14],
                matrix.values[19],
            ),
        }
    }
}
#[derive(Resource)]
struct Pipeline {
    layout: BindGroupLayoutDescriptor,
    fullscreen: FullscreenShader,
    shader: Handle<Shader>,
    sampler: Sampler,
}
fn init(
    mut commands: Commands,
    device: Res<RenderDevice>,
    assets: Res<AssetServer>,
    fullscreen: Res<FullscreenShader>,
) {
    commands.insert_resource(Pipeline {
        layout: BindGroupLayoutDescriptor::new(
            "fairygui composite",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    uniform_buffer::<CompositeUniform>(true),
                    texture_2d(TextureSampleType::Float { filterable: true }),
                ),
            ),
        ),
        fullscreen: fullscreen.clone(),
        shader: load_embedded_asset!(&*assets, "compositor.wesl"),
        sampler: device.create_sampler(&SamplerDescriptor::default()),
    });
}
#[derive(Default, Resource)]
struct Pipelines {
    composite: HashMap<(TextureFormat, Mode), CachedRenderPipelineId>,
    direct: HashMap<(CachedRenderPipelineId, Mode), CachedRenderPipelineId>,
}
impl Pipelines {
    fn composite(
        &mut self,
        format: TextureFormat,
        mode: Mode,
        pipeline: &Pipeline,
        cache: &mut PipelineCache,
    ) -> CachedRenderPipelineId {
        *self.composite.entry((format, mode)).or_insert_with(|| {
            let id = cache.queue_render_pipeline(RenderPipelineDescriptor {
                label: Some("fairygui composite".into()),
                layout: vec![pipeline.layout.clone()],
                vertex: pipeline.fullscreen.to_vertex_state(),
                fragment: Some(FragmentState {
                    shader: pipeline.shader.clone(),
                    targets: vec![Some(ColorTargetState {
                        format,
                        blend: Some(blend(mode, true)),
                        write_mask: ColorWrites::ALL,
                    })],
                    ..default()
                }),
                ..default()
            });
            cache.block_on_render_pipeline(id);
            id
        })
    }
    fn direct(
        &mut self,
        base: CachedRenderPipelineId,
        mode: Mode,
        cache: &mut PipelineCache,
    ) -> Option<CachedRenderPipelineId> {
        if let Some(id) = self.direct.get(&(base, mode)) {
            return Some(*id);
        }
        // Newly queued source descriptors become public after process_queue.
        cache.get_render_pipeline(base)?;
        let mut descriptor = cache.get_render_pipeline_descriptor(base).clone();
        for target in descriptor.fragment.as_mut()?.targets.iter_mut().flatten() {
            target.blend = Some(blend(mode, false));
        }
        let id = cache.queue_render_pipeline(descriptor);
        cache.block_on_render_pipeline(id);
        self.direct.insert((base, mode), id);
        Some(id)
    }
}

struct Surface {
    _texture: Texture,
    view: TextureView,
    binding: Option<BindGroup>,
    masked_binding: Option<BindGroup>,
}
impl Surface {
    fn new(device: &RenderDevice, size: Extent3d, format: TextureFormat) -> Self {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("fairygui isolation"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&TextureViewDescriptor::default());
        Self {
            _texture: texture,
            view,
            binding: None,
            masked_binding: None,
        }
    }
}
enum Op {
    Draw {
        range: Range<usize>,
        multiply: bool,
    },
    Group {
        group: Group,
        bounds: Rect,
        children: Vec<Op>,
        mask_children: Vec<Op>,
        uniform: u32,
    },
}
fn append(
    ops: &mut Vec<Op>,
    policy: &PaintPolicy,
    bounds: Rect,
    range: Range<usize>,
    depth: usize,
) {
    if let Some(group) = policy.groups.get(depth) {
        if !matches!(ops.last(), Some(Op::Group { group: last, .. }) if last.id == group.id) {
            ops.push(Op::Group {
                group: group.clone(),
                bounds,
                children: Vec::new(),
                mask_children: Vec::new(),
                uniform: 0,
            });
        }
        let Op::Group {
            children,
            mask_children,
            bounds: total,
            ..
        } = ops.last_mut().unwrap()
        else {
            unreachable!()
        };
        *total = total.union(bounds);
        let children = if policy.mask_for.contains(&group.id) {
            mask_children
        } else {
            children
        };
        append(children, policy, bounds, range, depth + 1);
    } else {
        let multiply = Mode::from(policy.blend) == Mode::Multiply;
        if !multiply
            && let Some(Op::Draw {
                range: previous,
                multiply: false,
            }) = ops.last_mut()
            && previous.end == range.start
        {
            previous.end = range.end;
        } else {
            ops.push(Op::Draw { range, multiply });
        }
    }
}
struct Frame {
    size: Extent3d,
    format: TextureFormat,
    layers: Vec<Surface>,
    masks: Vec<Option<Surface>>,
    scratch: Option<Surface>,
    uniforms: DynamicUniformBuffer<CompositeUniform>,
    multiply_uniform: u32,
    ops: Vec<Op>,
}
#[derive(Default, Resource)]
struct Frames(HashMap<RetainedViewEntity, Frame>);

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn prepare(
    policies: Res<Policies>,
    mut phases: ResMut<ViewSortedRenderPhases<TransparentUi>>,
    views: Query<(&ExtractedView, &UiViewTarget)>,
    targets: Query<&ViewTarget>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    pipeline: Res<Pipeline>,
    mut cache: ResMut<PipelineCache>,
    mut pipelines: ResMut<Pipelines>,
    mut frames: ResMut<Frames>,
) {
    if !policies.active {
        frames.0.clear();
        return;
    }
    frames
        .0
        .retain(|id, _| views.iter().any(|(v, _)| v.retained_view_entity == *id));
    let normal = PaintPolicy::default();
    for (view, target) in &views {
        let Some(phase) = phases.get_mut(&view.retained_view_entity) else {
            continue;
        };
        let Ok(target) = targets.get(target.0) else {
            continue;
        };
        let size = target.main_texture().size();
        let format = target.main_texture_format();
        let frame = frames
            .0
            .entry(view.retained_view_entity)
            .or_insert_with(|| Frame {
                size,
                format,
                layers: Vec::new(),
                masks: Vec::new(),
                scratch: None,
                uniforms: DynamicUniformBuffer::default(),
                multiply_uniform: 0,
                ops: Vec::new(),
            });
        if frame.size != size || frame.format != format {
            frame.layers.clear();
            frame.masks.clear();
            frame.scratch = None;
            frame.size = size;
            frame.format = format;
        }
        frame.ops.clear();
        frame.uniforms.clear();
        frame.multiply_uniform =
            frame
                .uniforms
                .push(&CompositeUniform::new(size, ColorMatrix::IDENTITY, 1.));
        let mut index = 0;
        let mut max_depth = 0;
        let mut multiply = false;
        while index < phase.items.len() {
            let item = &mut phase.items[index];
            let len = item.batch_range.len();
            if len == 0 {
                index += 1;
                continue;
            }
            let (policy, mut bounds) = policies
                .nodes
                .get(&item.entity.1)
                .map_or((&normal, Rect::default()), |(p, b)| (p, *b));
            max_depth = max_depth.max(policy.groups.len());
            let mode = Mode::from(policy.blend);
            multiply |= mode == Mode::Multiply;
            if matches!(mode, Mode::Add | Mode::Erase | Mode::Mask) {
                if let Some(id) = pipelines.direct(item.pipeline, mode, &mut cache) {
                    item.pipeline = id;
                } else {
                    // Wait for the source pipeline instead of displaying a normal-blend flash.
                    index += len;
                    continue;
                }
            }
            for next in phase
                .items
                .get_range(index + 1..(index + len).min(phase.items.len()))
                .unwrap()
                .values()
            {
                if let Some((_, other)) = policies.nodes.get(&next.entity.1) {
                    bounds = bounds.union(*other);
                }
            }
            append(&mut frame.ops, policy, bounds, index..index + len, 0);
            index += len;
        }
        frame.layers.truncate(max_depth);
        while frame.layers.len() < max_depth {
            frame.layers.push(Surface::new(&device, size, format));
        }
        let mut mask_depths = vec![false; max_depth];
        fn mask_depths_for(ops: &[Op], depth: usize, result: &mut [bool]) {
            for op in ops {
                if let Op::Group {
                    group,
                    children,
                    mask_children,
                    ..
                } = op
                {
                    result[depth] |= group.mask.is_some();
                    mask_depths_for(children, depth + 1, result);
                    mask_depths_for(mask_children, depth + 1, result);
                }
            }
        }
        mask_depths_for(&frame.ops, 0, &mut mask_depths);
        frame.masks.resize_with(max_depth, || None);
        for (surface, needed) in frame.masks.iter_mut().zip(mask_depths) {
            if needed {
                surface.get_or_insert_with(|| Surface::new(&device, size, format));
            } else {
                *surface = None;
            }
        }
        if multiply {
            frame
                .scratch
                .get_or_insert_with(|| Surface::new(&device, size, format));
        } else {
            frame.scratch = None;
        }
        fn uniforms(
            ops: &mut [Op],
            buffer: &mut DynamicUniformBuffer<CompositeUniform>,
            size: Extent3d,
            format: TextureFormat,
            pipelines: &mut Pipelines,
            pipeline: &Pipeline,
            cache: &mut PipelineCache,
        ) {
            for op in ops {
                match op {
                    Op::Group {
                        group,
                        children,
                        mask_children,
                        uniform,
                        ..
                    } => {
                        let mut data = CompositeUniform::new(size, group.matrix, group.opacity);
                        data.screen.w = if group.mask.is_some() { 1. } else { 0. };
                        *uniform = buffer.push(&data);
                        pipelines.composite(format, group.blend.into(), pipeline, cache);
                        uniforms(children, buffer, size, format, pipelines, pipeline, cache);
                        uniforms(
                            mask_children,
                            buffer,
                            size,
                            format,
                            pipelines,
                            pipeline,
                            cache,
                        );
                    }
                    Op::Draw { multiply: true, .. } => {
                        pipelines.composite(format, Mode::Multiply, pipeline, cache);
                    }
                    _ => (),
                }
            }
        }
        uniforms(
            &mut frame.ops,
            &mut frame.uniforms,
            size,
            format,
            &mut pipelines,
            &pipeline,
            &mut cache,
        );
        frame.uniforms.write_buffer(&device, &queue);
        let layout = cache.get_bind_group_layout(&pipeline.layout);
        for surface in frame.layers.iter_mut().chain(frame.scratch.iter_mut()) {
            surface.binding = Some(device.create_bind_group(
                "fairygui composite",
                &layout,
                &BindGroupEntries::sequential((
                    &surface.view,
                    &pipeline.sampler,
                    frame.uniforms.binding().unwrap(),
                    &surface.view,
                )),
            ));
        }
        for (surface, mask) in frame.layers.iter_mut().zip(&frame.masks) {
            surface.masked_binding = mask.as_ref().map(|mask| {
                device.create_bind_group(
                    "fairygui alpha mask",
                    &layout,
                    &BindGroupEntries::sequential((
                        &surface.view,
                        &pipeline.sampler,
                        frame.uniforms.binding().unwrap(),
                        &mask.view,
                    )),
                )
            });
        }
    }
}

struct Painter<'w> {
    world: &'w World,
    phase: &'w SortedRenderPhase<TransparentUi>,
    view: Entity,
    camera: &'w ExtractedCamera,
    frame: &'w Frame,
    pipelines: &'w Pipelines,
    cache: &'w PipelineCache,
}
fn attachment(view: &TextureView, clear: bool) -> RenderPassColorAttachment<'_> {
    RenderPassColorAttachment {
        view,
        depth_slice: None,
        resolve_target: None,
        ops: Operations {
            load: if clear {
                LoadOp::Clear(bevy::color::LinearRgba::NONE.into())
            } else {
                LoadOp::Load
            },
            store: StoreOp::Store,
        },
    }
}
impl Painter<'_> {
    fn draw(
        &self,
        ctx: &mut RenderContext,
        target: &TextureView,
        range: Range<usize>,
        clear: bool,
    ) {
        let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("fairygui UI"),
            color_attachments: &[Some(attachment(target, clear))],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        if let Some(viewport) = self.camera.viewport.as_ref() {
            pass.set_camera_viewport(viewport);
        }
        if let Err(error) = self
            .phase
            .render_range(&mut pass, self.world, self.view, range)
        {
            error!(?error, "FairyGUI UI draw failed");
        }
    }
    fn composite(
        &self,
        ctx: &mut RenderContext,
        target: &TextureView,
        binding: &BindGroup,
        mode: Mode,
        uniform: u32,
        bounds: Option<Rect>,
    ) {
        let Some(id) = self.pipelines.composite.get(&(self.frame.format, mode)) else {
            return;
        };
        let Some(pipeline) = self.cache.get_render_pipeline(*id) else {
            return;
        };
        let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("fairygui composite"),
            color_attachments: &[Some(attachment(target, false))],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        if let Some(viewport) = self.camera.viewport.as_ref() {
            pass.set_camera_viewport(viewport);
        }
        if let Some(bounds) = bounds {
            let offset = self
                .camera
                .viewport
                .as_ref()
                .map_or(Vec2::ZERO, |v| v.physical_position.as_vec2());
            let bounds = Rect::from_corners(bounds.min + offset, bounds.max + offset);
            let min = bounds.min.floor().max(Vec2::ZERO).as_uvec2();
            let max = bounds
                .max
                .ceil()
                .min(Vec2::new(
                    self.frame.size.width as f32,
                    self.frame.size.height as f32,
                ))
                .max(min.as_vec2())
                .as_uvec2();
            if max.x <= min.x || max.y <= min.y {
                return;
            }
            pass.set_scissor_rect(min.x, min.y, max.x - min.x, max.y - min.y);
        }
        pass.set_render_pipeline(pipeline);
        pass.set_bind_group(0, binding, &[uniform]);
        pass.draw(0..3, 0..1);
    }
    fn paint(
        &self,
        ctx: &mut RenderContext,
        target: &TextureView,
        ops: &[Op],
        depth: usize,
        clear: bool,
    ) {
        if clear {
            self.draw(ctx, target, 0..0, true);
        }
        for op in ops {
            match op {
                Op::Draw {
                    range,
                    multiply: false,
                } => self.draw(ctx, target, range.clone(), false),
                Op::Draw {
                    range,
                    multiply: true,
                } => {
                    let scratch = self.frame.scratch.as_ref().unwrap();
                    self.draw(ctx, &scratch.view, range.clone(), true);
                    self.composite(
                        ctx,
                        target,
                        scratch.binding.as_ref().unwrap(),
                        Mode::Multiply,
                        self.frame.multiply_uniform,
                        None,
                    );
                }
                Op::Group {
                    group,
                    bounds,
                    children,
                    mask_children,
                    uniform,
                } => {
                    // Laya skips almost-transparent offscreen substructures.
                    if group.opacity < 0.01 {
                        continue;
                    }
                    let layer = &self.frame.layers[depth];
                    if let Some(mask) = self.frame.masks[depth].as_ref()
                        && group.mask.is_some()
                    {
                        self.paint(ctx, &mask.view, mask_children, depth + 1, true);
                    }
                    self.paint(ctx, &layer.view, children, depth + 1, true);
                    let mode = Mode::from(group.blend);
                    self.composite(
                        ctx,
                        target,
                        if group.mask.is_some() {
                            layer.masked_binding.as_ref().unwrap()
                        } else {
                            layer.binding.as_ref().unwrap()
                        },
                        mode,
                        *uniform,
                        // Only destination-in changes transparent pixels outside
                        // the source. Other modes must preserve overflowing glyphs.
                        (mode == Mode::Mask).then_some(*bounds),
                    );
                }
            }
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn composite_pass(
    world: &World,
    view: ViewQuery<&UiCameraView>,
    ui_views: Query<(&ExtractedView, &UiViewTarget)>,
    targets: Query<(&ViewTarget, &ExtractedCamera)>,
    phases: Res<ViewSortedRenderPhases<TransparentUi>>,
    frames: Res<Frames>,
    pipelines: Res<Pipelines>,
    cache: Res<PipelineCache>,
    mut ctx: RenderContext,
) {
    let ui_view = view.into_inner().0;
    let Ok((extracted, target)) = ui_views.get(ui_view) else {
        return;
    };
    let Ok((target, camera)) = targets.get(target.0) else {
        return;
    };
    let Some(phase) = phases.get(&extracted.retained_view_entity) else {
        return;
    };
    let Some(frame) = frames.0.get(&extracted.retained_view_entity) else {
        return;
    };
    // Honor the camera target's first-use clear even when the scene is empty.
    drop(ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("fairygui target"),
        color_attachments: &[Some(target.get_unsampled_color_attachment())],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    }));
    Painter {
        world,
        phase,
        view: ui_view,
        camera,
        frame,
        pipelines: &pipelines,
        cache: &cache,
    }
    .paint(&mut ctx, target.main_texture_view(), &frame.ops, 0, false);
}

pub(super) fn install(app: &mut App) {
    embedded_asset!(app, "compositor.wesl");
    let render = app.sub_app_mut(RenderApp);
    render
        .init_resource::<Policies>()
        .init_resource::<Pipelines>()
        .init_resource::<Frames>()
        .add_systems(RenderStartup, init)
        .add_systems(ExtractSchedule, extract_policies)
        .add_systems(
            Render,
            (
                barriers
                    .after(RenderSystems::PhaseSort)
                    .before(RenderSystems::Prepare),
                prepare
                    .after(RenderSystems::PrepareBindGroups)
                    .before(RenderSystems::Render),
            ),
        );
    render
        .world_mut()
        .schedule_scope(Core2d, |world, schedule| {
            schedule
                .remove_systems_in_set(ui_pass, world, ScheduleCleanupPolicy::RemoveSystemsOnly)
                .expect("replace Bevy 2D UI pass");
        });
    render
        .world_mut()
        .schedule_scope(Core3d, |world, schedule| {
            schedule
                .remove_systems_in_set(ui_pass, world, ScheduleCleanupPolicy::RemoveSystemsOnly)
                .expect("replace Bevy 3D UI pass");
        });
    render
        .add_systems(
            Core2d,
            (
                ui_pass
                    .run_if(no_effects)
                    .after(Core2dSystems::PostProcess)
                    .before(upscaling),
                composite_pass
                    .run_if(effects)
                    .after(Core2dSystems::PostProcess)
                    .before(upscaling),
            ),
        )
        .add_systems(
            Core3d,
            (
                ui_pass
                    .run_if(no_effects)
                    .after(Core3dSystems::PostProcess)
                    .before(upscaling),
                composite_pass
                    .run_if(effects)
                    .after(Core3dSystems::PostProcess)
                    .before(upscaling),
            ),
        );
}
