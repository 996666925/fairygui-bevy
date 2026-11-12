//! Native LayaAir 3.3 nine-grid layout, UV bounds and cropped tile tails.
//! Ported from display/mesh/MeshFactory.ts; see LICENSE-LayaAir.
use crate::geom::{Rect, Size};

/// Native mesh bits: top, bottom, left, right, center. Corners never tile.
pub const GRID_PARTS: [i8; 9] = [-1, 0, -1, 2, 4, 3, -1, 1, -1];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TileGridCell {
    pub dest: Rect,
    pub src: Rect,
    pub tiled: bool,
    /// Native tile step: edges repeat along their long axis only.
    pub tile_size: Size,
}

/// Independent border compression along one target axis. Native corners keep
/// their width/height until that axis is smaller than the two fixed borders.
fn target_axis(source: f64, start: f64, end: f64, target: f64) -> [f64; 4] {
    if target >= start + source - end {
        [0.0, start, target - (source - end), target]
    } else {
        let ratio = start / (source - end);
        let middle = target * ratio / (1.0 + ratio);
        [0.0, middle, middle, target]
    }
}

/// Row-major cells, using the five-bit MeshFactory mask. FairyGUI-Laya's
/// tileGridIndice property is normalized separately: any nonzero value -> 31.
pub fn split(grid: Rect, source: Size, target: Size, indice: i32) -> Vec<TileGridCell> {
    if source.is_zero() || target.is_zero() {
        return Vec::new();
    }
    let xs = target_axis(
        f64::from(source.width),
        f64::from(grid.x),
        f64::from(grid.right()),
        f64::from(target.width),
    )
    .map(|n| n as f32);
    let ys = target_axis(
        f64::from(source.height),
        f64::from(grid.y),
        f64::from(grid.bottom()),
        f64::from(target.height),
    )
    .map(|n| n as f32);
    let sx = [0.0, grid.x, grid.right(), source.width];
    let sy = [0.0, grid.y, grid.bottom(), source.height];
    (0..9)
        .map(|index| {
            let row = index / 3;
            let col = index % 3;
            let part = GRID_PARTS[index];
            let dest = Rect::new(
                xs[col],
                ys[row],
                xs[col + 1] - xs[col],
                ys[row + 1] - ys[row],
            );
            TileGridCell {
                dest,
                src: Rect::new(
                    sx[col],
                    sy[row],
                    sx[col + 1] - sx[col],
                    sy[row + 1] - sy[row],
                ),
                tiled: part >= 0 && indice & (1 << part) != 0,
                tile_size: Size::new(
                    if matches!(part, 0 | 1 | 4) {
                        grid.width
                    } else {
                        dest.width
                    },
                    if matches!(part, 2..=4) {
                        grid.height
                    } else {
                        dest.height
                    },
                ),
            }
        })
        .collect()
}

/// Number of copies including a cropped final copy, not a stretched last tile.
pub fn tile_count(cell: &TileGridCell, vertical: bool) -> u32 {
    let (step, dest) = if vertical {
        (cell.tile_size.height, cell.dest.height)
    } else {
        (cell.tile_size.width, cell.dest.width)
    };
    if step <= 0.0 || dest <= 0.0 {
        0
    } else if !cell.tiled {
        1
    } else {
        (dest / step).ceil() as u32
    }
}

/// Tile rectangles and source fractions as emitted by native genTileMesh.
/// Full copies precede a cropped tail. Zero-area cells do not emit quads.
pub fn tiles(cell: &TileGridCell) -> Vec<(Rect, Rect)> {
    if cell.dest.is_empty() || cell.src.is_empty() {
        return Vec::new();
    }
    if !cell.tiled {
        return vec![(cell.dest, cell.src)];
    }
    let nx = tile_count(cell, false);
    let ny = tile_count(cell, true);
    let mut result = Vec::new();
    for x in 0..nx {
        for y in 0..ny {
            let dx = x as f32 * cell.tile_size.width;
            let dy = y as f32 * cell.tile_size.height;
            let width = (cell.dest.width - dx).min(cell.tile_size.width);
            let height = (cell.dest.height - dy).min(cell.tile_size.height);
            result.push((
                Rect::new(cell.dest.x + dx, cell.dest.y + dy, width, height),
                Rect::new(
                    cell.src.x,
                    cell.src.y,
                    cell.src.width * width / cell.tile_size.width,
                    cell.src.height * height / cell.tile_size.height,
                ),
            ));
        }
    }
    result
}

/// The source virtual UV rect can extend past its cropped atlas rectangle.
#[derive(Debug, Clone, Copy)]
pub struct GridTexture {
    pub source: Size,
    pub virtual_uv: Rect,
    pub clip_uv: Rect,
    pub atlas: Size,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridAxes {
    pub x: [f64; 4],
    pub y: [f64; 4],
    pub u: [f64; 4],
    pub v: [f64; 4],
}
#[allow(clippy::too_many_arguments)] // Preserve native atlas-edge parameters.
fn texture_axis(
    source: f64,
    start: f64,
    end: f64,
    uv_start: f64,
    uv_size: f64,
    clip_start: f64,
    clip_size: f64,
    atlas: f64,
) -> [f64; 4] {
    let scale = uv_size / source;
    let mut axis = [
        uv_start,
        uv_start + start * scale,
        uv_start + end * scale,
        uv_start + uv_size,
    ];
    let half = (0.5 / atlas).min(clip_size * 0.5);
    let inset = |value: f64| {
        if (value - clip_start).abs() <= 1e-7 {
            clip_start + half
        } else if (value - clip_start - clip_size).abs() <= 1e-7 {
            clip_start + clip_size - half
        } else {
            value
        }
    };
    if start.abs() <= 1e-7 {
        axis[0] = inset(axis[0]);
        axis[1] = inset(axis[1]);
    }
    if (source - end).abs() <= 1e-7 {
        axis[2] = inset(axis[2]);
        axis[3] = inset(axis[3]);
    }
    axis
}

/// MeshFactory grid axes, including independent small-target compression and
/// half-texel UV inset when a zero border reaches a cropped atlas edge.
pub fn axes(grid: Rect, target: Size, texture: GridTexture) -> GridAxes {
    let source = texture.source;
    let uv = texture.virtual_uv;
    let clip = texture.clip_uv;
    let mut result = GridAxes {
        x: target_axis(
            f64::from(source.width),
            f64::from(grid.x),
            f64::from(grid.right()),
            f64::from(target.width),
        ),
        y: target_axis(
            f64::from(source.height),
            f64::from(grid.y),
            f64::from(grid.bottom()),
            f64::from(target.height),
        ),
        u: [0.0; 4],
        v: [0.0; 4],
    };
    // Native only applies the inset for subtextures, not a full standalone image.
    if clip == Rect::new(0.0, 0.0, 1.0, 1.0) {
        result.u = [
            f64::from(uv.x),
            f64::from(uv.x) + f64::from(grid.x) * f64::from(uv.width) / f64::from(source.width),
            f64::from(uv.x)
                + f64::from(grid.right()) * f64::from(uv.width) / f64::from(source.width),
            f64::from(uv.x) + f64::from(uv.width),
        ];
        result.v = [
            f64::from(uv.y),
            f64::from(uv.y) + f64::from(grid.y) * f64::from(uv.height) / f64::from(source.height),
            f64::from(uv.y)
                + f64::from(grid.bottom()) * f64::from(uv.height) / f64::from(source.height),
            f64::from(uv.y) + f64::from(uv.height),
        ];
    } else {
        result.u = texture_axis(
            f64::from(source.width),
            f64::from(grid.x),
            f64::from(grid.right()),
            f64::from(uv.x),
            f64::from(uv.width),
            f64::from(clip.x),
            f64::from(clip.width),
            f64::from(texture.atlas.width),
        );
        result.v = texture_axis(
            f64::from(source.height),
            f64::from(grid.y),
            f64::from(grid.bottom()),
            f64::from(uv.y),
            f64::from(uv.height),
            f64::from(clip.y),
            f64::from(clip.height),
            f64::from(texture.atlas.height),
        );
    }
    result
}

#[cfg(test)]
#[path = "tile_grid_tests.rs"]
mod tests;
