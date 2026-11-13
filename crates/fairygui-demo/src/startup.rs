//! Reveal the native window only after a complete menu frame reaches the GPU.
use super::*;
use bevy::render::{
    Render, RenderApp,
    extract_resource::{ExtractResource, ExtractResourcePlugin},
    render_resource::{CachedPipelineState, PipelineCache},
    renderer::{RenderQueue, render_system},
    view::window::ExtractedWindow,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Resource, Clone, ExtractResource, Default)]
#[extract_app(RenderApp)]
struct Presentation {
    menu_ready: bool,
    submitted: Arc<AtomicBool>,
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<Presentation>()
        .add_plugins(ExtractResourcePlugin::<Presentation>::default())
        .add_systems(First, reveal)
        .add_systems(PostUpdate, prepare.after(bevy::ui::UiSystems::PostLayout));
    app.sub_app_mut(RenderApp)
        .add_systems(Render, acknowledge.after(render_system));
}

#[allow(clippy::too_many_arguments)]
fn prepare(
    demo: Res<Demo>,
    gui: GuiQuery,
    font: Res<GuiFont>,
    assets: Res<AssetServer>,
    images: Res<Assets<Image>>,
    image_nodes: Query<&ImageNode>,
    text: Query<(&Text, &bevy::text::TextLayoutInfo)>,
    nodes: Query<&ComputedNode>,
    mut presentation: ResMut<Presentation>,
) {
    if presentation.menu_ready {
        return;
    }
    let Some(menu) = demo.menu_owner.filter(|_| demo.menu.is_some()) else {
        return;
    };
    if !font
        .0
        .as_ref()
        .is_some_and(|font| assets.is_loaded_with_dependencies(font))
    {
        return;
    }
    let Some(root) = gui.view(menu) else { return };
    let mut pending = vec![root];
    while let Some(entity) = pending.pop() {
        let Some(info) = gui.get(entity) else { return };
        pending.extend_from_slice(info.children());
        if !gui
            .display(entity)
            .is_some_and(|display| display.display_visible() && display.on_stage())
        {
            continue;
        }
        if let Ok(image) = image_nodes.get(entity)
            && !images.contains(image.image.id())
        {
            return;
        }
        if let Ok((text, layout)) = text.get(entity)
            && !text.0.is_empty()
            && (layout.glyphs.is_empty() || layout.size.min_element() <= 0.)
        {
            return;
        }
        if nodes.get(entity).is_err() {
            return;
        }
    }
    presentation.menu_ready = true;
}

fn acknowledge(
    presentation: Res<Presentation>,
    pipelines: Res<PipelineCache>,
    queue: Res<RenderQueue>,
    windows: Query<&ExtractedWindow>,
    mut submitted: Local<bool>,
) {
    if *submitted
        || !presentation.menu_ready
        || pipelines.pipelines().next().is_none()
        || !pipelines
            .pipelines()
            .all(|pipeline| matches!(pipeline.state, CachedPipelineState::Ok(_)))
        || !windows
            .iter()
            .any(|window| !window.needs_initial_present && window.swap_chain_texture.is_none())
    {
        return;
    }
    *submitted = true;
    let ready = presentation.submitted.clone();
    queue.on_submitted_work_done(move || ready.store(true, Ordering::Release));
}

fn reveal(
    presentation: Res<Presentation>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    mut revealed: Local<bool>,
) {
    if !*revealed
        && presentation.submitted.load(Ordering::Acquire)
        && let Ok(mut window) = windows.single_mut()
    {
        window.visible = true;
        *revealed = true;
        info!("Startup menu frame presented; showing the demo window");
    }
}
