//! Writable GGraph drawing parameters using Bevy colors and paired coordinates.
use crate::{
    GraphType, GuiRuntime, backend,
    core::{
        object::GObjectId,
        widget::{ControlData, GraphData},
    },
    properties::Property,
    text_format::native_color,
};
use bevy::prelude::*;

/// Graph drawing configuration in the existing object's local coordinates;
/// drawing may extend beyond its rectangle. Position/size use GuiTransform.
/// Fill also has the GuiColor
/// alias: GuiColor captures its color gear, drawing configuration does not.
#[derive(Component, Debug, Clone)]
pub struct GuiGraph {
    pub kind: GraphType,
    pub line_size: i32,
    pub line_color: Color,
    pub fill_color: Color,
    /// Native order: top-left, top-right, bottom-left, bottom-right.
    pub corner_radius: Option<[f32; 4]>,
    /// Explicit local coordinates, unchanged when the object resizes.
    pub polygon_points: Vec<Vec2>,
    pub sides: i32,
    /// Degrees in screen coordinates.
    pub start_angle: f32,
    /// Per-vertex regular-polygon coefficients. Missing/NaN entries mean 1.
    pub distances: Vec<f32>,
}
fn number_eq(a: f32, b: f32) -> bool {
    a == b || a.is_nan() && b.is_nan()
}
fn numbers_eq(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(&a, &b)| number_eq(a, b))
}
fn points_eq(a: &[Vec2], b: &[Vec2]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(a, b)| number_eq(a.x, b.x) && number_eq(a.y, b.y))
}
fn corners_eq(a: Option<[f32; 4]>, b: Option<[f32; 4]>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => numbers_eq(&a, &b),
        (None, None) => true,
        _ => false,
    }
}
impl PartialEq for GuiGraph {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.line_size == other.line_size
            && self.line_color == other.line_color
            && self.fill_color == other.fill_color
            && corners_eq(self.corner_radius, other.corner_radius)
            && points_eq(&self.polygon_points, &other.polygon_points)
            && self.sides == other.sides
            && number_eq(self.start_angle, other.start_angle)
            && numbers_eq(&self.distances, &other.distances)
    }
}
impl Default for GuiGraph {
    fn default() -> Self {
        Self::from_native(&GraphData::default())
    }
}
impl GuiGraph {
    fn from_native(data: &GraphData) -> Self {
        Self {
            kind: data.graph_kind(),
            line_size: data.line_size,
            line_color: backend::to_bevy_color(data.line_color),
            fill_color: backend::to_bevy_color(data.fill_color),
            corner_radius: data.corner_radius,
            polygon_points: data
                .polygon_points
                .as_deref()
                .unwrap_or_default()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|p| Vec2::new(p[0], p[1]))
                .collect(),
            sides: data.sides,
            start_angle: data.start_angle,
            distances: data.distances.clone().unwrap_or_default(),
        }
    }
}
impl Property for GuiGraph {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        let ControlData::Graph(data) = runtime.store().object(object)?.control() else {
            return None;
        };
        Some(Self::from_native(data))
    }
    fn apply(&self, previous: &Self, _: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        let Some(ControlData::Graph(data)) = runtime.store().object(object).map(|o| o.control())
        else {
            return;
        };
        let mut next = data.clone();
        if self.kind != previous.kind {
            next.graph_type = self.kind as i32;
        }
        if self.line_size != previous.line_size {
            next.line_size = self.line_size;
        }
        if self.line_color != previous.line_color
            && let Some(color) = native_color(self.line_color)
        {
            next.line_color = color;
        }
        if self.fill_color != previous.fill_color
            && let Some(color) = native_color(self.fill_color)
        {
            next.fill_color = color;
        }
        if !corners_eq(self.corner_radius, previous.corner_radius)
            && self
                .corner_radius
                .is_none_or(|v| v.iter().all(|v| v.is_finite()))
        {
            next.corner_radius = self.corner_radius;
        }
        if !points_eq(&self.polygon_points, &previous.polygon_points)
            && self.polygon_points.iter().all(|p| p.is_finite())
        {
            next.polygon_points = Some(
                self.polygon_points
                    .iter()
                    .flat_map(|p| [p.x, p.y])
                    .collect(),
            );
        }
        if self.sides != previous.sides {
            next.sides = self.sides;
        }
        if !number_eq(self.start_angle, previous.start_angle) && self.start_angle.is_finite() {
            next.start_angle = self.start_angle;
        }
        if !numbers_eq(&self.distances, &previous.distances)
            && self.distances.iter().all(|v| v.is_finite() || v.is_nan())
        {
            next.distances = Some(self.distances.clone());
        }
        runtime.store_mut().set_graph(object, next);
    }
}

#[cfg(test)]
#[path = "ecs_graph_properties_tests.rs"]
mod tests;
