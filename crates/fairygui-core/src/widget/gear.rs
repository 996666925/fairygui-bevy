//! 齿轮：随控制器页面变化而联动的属性。
//!
//! 一个对象可以挂多个齿轮，每个齿轮绑定容器的某个控制器。控制器切页时，
//! 齿轮把对应页面记录下来的属性值套用到对象上，这是界面「状态」的主要实现方式。
//!
//! 描述格式由 `GearBase.setup` 统一驱动，各类齿轮只在取值部分不同。有几处
//! 容易写错的地方在此特别注明：
//!
//! - 坐标齿轮的坐标用 `getInt32` 读，百分比扩展值才用 `getFloat32`；
//! - 外观齿轮的透明度与旋转是 `getFloat32`，而置灰与可点击是 `readBool`；
//! - 可见性齿轮不记录取值，而是直接记录页面编号数组；
//! - 只有坐标齿轮有百分比扩展，只有第二个可见性齿轮有条件字节。

use std::collections::HashMap;

use crate::binary::{ByteBuffer, StringTable};
use crate::color::Color;
use crate::error::Result;
use crate::object::{GObjectId, ObjectStore};

/// 齿轮种类。编号顺序与官方 `GearBase.create` 的分派表一致。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GearKind {
    /// 控制可见性。
    Display,
    /// 坐标。
    Xy,
    /// 尺寸与缩放。
    Size,
    /// 外观：透明度、旋转、置灰、可点击。
    Look,
    /// 颜色。
    Color,
    /// 播放状态与当前帧。
    Animation,
    /// 文本。
    Text,
    /// 图标。
    Icon,
    /// 带条件的可见性。
    Display2,
    /// 字号。
    FontSize,
}

impl GearKind {
    /// 由描述中的编号构造。
    pub const fn from_code(code: u8) -> Self {
        match code {
            1 => Self::Xy,
            2 => Self::Size,
            3 => Self::Look,
            4 => Self::Color,
            5 => Self::Animation,
            6 => Self::Text,
            7 => Self::Icon,
            8 => Self::Display2,
            9 => Self::FontSize,
            _ => Self::Display,
        }
    }
}

/// 属性变化时的缓动配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GearTweenConfig {
    /// 缓动类型编号。
    pub ease_type: i32,
    /// 时长（秒）。
    pub duration: f32,
    /// 延迟（秒）。
    pub delay: f32,
}

impl Default for GearTweenConfig {
    fn default() -> Self {
        Self {
            ease_type: 10,
            duration: 0.3,
            delay: 0.0,
        }
    }
}

/// 某个页面下记录的一组属性值。
#[derive(Debug, Clone, PartialEq)]
pub enum GearValue {
    /// 坐标，含百分比坐标。
    Xy { x: f32, y: f32, px: f32, py: f32 },
    /// 尺寸与缩放。
    Size {
        width: f32,
        height: f32,
        scale_x: f32,
        scale_y: f32,
    },
    /// 外观。
    Look {
        alpha: f32,
        rotation: f32,
        grayed: bool,
        touchable: bool,
    },
    /// 颜色与描边色。
    Color { color: Color, stroke_color: Color },
    /// 播放状态与当前帧。
    Animation { playing: bool, frame: i32 },
    /// 文本。
    Text(String),
    /// 图标。
    Icon(String),
    /// 字号。
    FontSize(i32),
}

/// 一个齿轮。
#[derive(Debug, Clone)]
pub struct Gear {
    /// 种类。
    pub kind: GearKind,
    /// 绑定的控制器编号（在所属容器控制器列表中的位置），`-1` 表示未绑定。
    pub controller_index: i32,
    /// Owner of the bound controller, retained when the object is reparented.
    /// Parsed gears bind at setup; None is available for newly constructed gears.
    pub controller_owner: Option<GObjectId>,
    /// 可见性齿轮关心的页面编号。
    pub pages: Vec<String>,
    /// 可见性齿轮的条件。
    pub condition: i32,
    /// 坐标是否以百分比记录。
    pub positions_in_percent: bool,
    /// 缓动配置，为空表示直接切换。
    pub tween: Option<GearTweenConfig>,
    /// 各页面的取值。`None` 键表示默认值。
    pub values: HashMap<Option<String>, GearValue>,
    /// Runtime storage for selectedPageId=null, distinct from the default value.
    pub unselected_value: Option<GearValue>,
}

impl Gear {
    /// 新建一个指定种类的空齿轮。
    pub fn new(kind: GearKind) -> Self {
        Self {
            kind,
            controller_index: -1,
            controller_owner: None,
            pages: Vec::new(),
            condition: 0,
            positions_in_percent: false,
            tween: None,
            values: HashMap::new(),
            unselected_value: None,
        }
    }

    /// 读取齿轮描述。
    ///
    /// `buffer` 的位置应当是该齿轮块的起点。
    pub(crate) fn setup(&mut self, buffer: &mut ByteBuffer<'_>, table: &StringTable) -> Result<()> {
        self.controller_index = i32::from(buffer.read_i16()?);
        let count = buffer.read_i16()?.max(0) as usize;

        match self.kind {
            // 可见性齿轮只记录页面编号，不记录取值。
            GearKind::Display | GearKind::Display2 => {
                self.pages = buffer
                    .read_s_array(table, count)?
                    .into_iter()
                    .map(Option::unwrap_or_default)
                    .collect();
            }
            _ => {
                for _ in 0..count {
                    let Some(page) = buffer.read_s(table)? else {
                        continue;
                    };
                    let value = self.read_value(buffer, table)?;
                    self.values.insert(Some(page), value);
                }
                if buffer.read_bool()? {
                    let value = self.read_value(buffer, table)?;
                    self.values.insert(None, value);
                }
            }
        }

        if buffer.read_bool()? {
            let ease_type = i32::from(buffer.read_i8()?);
            let duration = buffer.read_f32()?;
            let delay = buffer.read_f32()?;
            self.tween = Some(GearTweenConfig {
                ease_type,
                duration,
                delay,
            });
        }

        if buffer.version() >= 2 {
            match self.kind {
                GearKind::Xy => {
                    if buffer.read_bool()? {
                        self.positions_in_percent = true;
                        for _ in 0..count {
                            let Some(page) = buffer.read_s(table)? else {
                                continue;
                            };
                            self.read_percent_value(buffer, Some(page))?;
                        }
                        if buffer.read_bool()? {
                            self.read_percent_value(buffer, None)?;
                        }
                    }
                }
                GearKind::Display2 => {
                    self.condition = i32::from(buffer.read_i8()?);
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// 按控制器的当前页面把取值套用到对象上。
    ///
    /// 该页面没有记录取值时回落到默认值；默认值也没有记录时不动对象。
    ///
    /// 配置了缓动且该齿轮支持插值时，这里**不会立刻设值**，而是返回一个补间
    /// 请求，交由调用方逐帧推进。可见性齿轮总是立即生效——对显隐做插值没有意义。
    pub fn apply(
        &self,
        store: &mut ObjectStore,
        owner: GObjectId,
        page: Option<&str>,
    ) -> Option<crate::object::GearTween> {
        if matches!(self.kind, GearKind::Display | GearKind::Display2) {
            return self.apply_locked(store, owner, page);
        }
        store.with_gear_lock(owner, |store| self.apply_locked(store, owner, page))
    }

    fn apply_locked(
        &self,
        store: &mut ObjectStore,
        owner: GObjectId,
        page: Option<&str>,
    ) -> Option<crate::object::GearTween> {
        if matches!(self.kind, GearKind::Display | GearKind::Display2) {
            let visible =
                self.pages.is_empty() || page.is_some_and(|id| self.pages.iter().any(|p| p == id));
            store.apply_display_gear(
                owner,
                self.kind,
                self.controller_index,
                visible,
                self.condition,
            );
            return None;
        }

        let mut value = page
            .and_then(|page| self.values.get(&Some(page.to_owned())))
            .or_else(|| {
                if page.is_none() {
                    self.unselected_value.as_ref()
                } else {
                    None
                }
            })
            .or_else(|| self.values.get(&None))?
            .clone();
        if self.kind == GearKind::Xy
            && self.positions_in_percent
            && let GearValue::Xy { x, y, px, py } = &mut value
            && let Some(parent) = store.object(owner).and_then(|object| object.parent())
            && let Some(parent) = store.object(parent)
        {
            *x = *px * parent.width();
            *y = *py * parent.height();
        }

        let active = store.take_gear_tween(owner, self.kind);
        let can_tween = self
            .tween
            .as_ref()
            .is_some_and(|config| config.duration > 0.0)
            && snapshot_value(store, owner, self.kind)
                .is_some_and(|start| same_shape(&start, &value));
        if let Some(mut active) = active {
            if can_tween {
                if active.end.same_tween_target(&value) {
                    // Look's boolean flags change immediately, even when the numeric target is unchanged.
                    if let GearValue::Look {
                        grayed, touchable, ..
                    } = &value
                    {
                        store.set_grayed(owner, *grayed);
                        store.set_touchable(owner, *touchable);
                        active.start.copy_look_flags(&value);
                        active.end.copy_look_flags(&value);
                    }
                    return Some(active);
                }
                // Native gears complete the previous tween before changing destination.
                active.end.apply(store, owner, self.kind);
            }
            if let Some(token) = active.display_lock {
                store.release_display_lock(owner, token);
            }
        }

        if let GearValue::Look {
            grayed, touchable, ..
        } = &value
        {
            store.set_grayed(owner, *grayed);
            store.set_touchable(owner, *touchable);
        }

        // 只有能读出当前值的齿轮才谈得上「从当前值过渡到目标值」。
        if let Some(config) = &self.tween
            && config.duration > 0.0
            && let Some(start) = snapshot_value(store, owner, self.kind)
            && same_shape(&start, &value)
            && !start.same_tween_target(&value)
        {
            return Some(crate::object::GearTween {
                owner,
                kind: self.kind,
                start,
                end: value.clone(),
                elapsed: 0.0,
                delay: config.delay.max(0.0),
                duration: config.duration,
                ease_type: config.ease_type,
                display_lock: store.lock_gear_display(owner, self.controller_index),
            });
        }

        // 颜色、文本、图标与字号要改动控件专有数据，单独分派；
        // 其余通用属性交给取值本身处理。
        match self.kind {
            GearKind::Color => {
                if let GearValue::Color {
                    color,
                    stroke_color,
                } = &value
                {
                    store.set_control_color(owner, *color, Some(*stroke_color));
                }
            }
            GearKind::Text => {
                if let GearValue::Text(text) = &value {
                    self.apply_text(store, owner, text);
                }
            }
            GearKind::Icon => {
                if let GearValue::Icon(icon) = &value {
                    self.apply_icon(store, owner, icon);
                }
            }
            GearKind::FontSize => {
                if let GearValue::FontSize(size) = &value {
                    self.apply_font_size(store, owner, *size);
                }
            }
            GearKind::Animation => {
                if let GearValue::Animation { playing, frame } = value {
                    store.set_animation_playing(owner, playing);
                    store.set_animation_frame(owner, frame);
                }
            }
            _ => value.apply(store, owner, self.kind),
        }

        None
    }

    fn apply_text(&self, store: &mut ObjectStore, owner: GObjectId, text: &str) {
        store.set_object_text(owner, text);
    }

    fn apply_icon(&self, store: &mut ObjectStore, owner: GObjectId, icon: &str) {
        store.apply_icon_to(owner, icon);
    }

    fn apply_font_size(&self, store: &mut ObjectStore, owner: GObjectId, size: i32) {
        store.set_object_font_size(owner, size);
    }

    fn read_value(
        &mut self,
        buffer: &mut ByteBuffer<'_>,
        table: &StringTable,
    ) -> Result<GearValue> {
        Ok(match self.kind {
            GearKind::Xy => GearValue::Xy {
                x: buffer.read_i32()? as f32,
                y: buffer.read_i32()? as f32,
                px: 0.0,
                py: 0.0,
            },
            GearKind::Size => GearValue::Size {
                width: buffer.read_i32()? as f32,
                height: buffer.read_i32()? as f32,
                scale_x: buffer.read_f32()?,
                scale_y: buffer.read_f32()?,
            },
            GearKind::Look => GearValue::Look {
                alpha: buffer.read_f32()?,
                rotation: buffer.read_f32()?,
                grayed: buffer.read_bool()?,
                touchable: buffer.read_bool()?,
            },
            GearKind::Color => GearValue::Color {
                color: read_color(buffer)?,
                stroke_color: read_color(buffer)?,
            },
            GearKind::Animation => GearValue::Animation {
                playing: buffer.read_bool()?,
                frame: buffer.read_i32()?,
            },
            GearKind::Text => GearValue::Text(buffer.read_s(table)?.unwrap_or_default()),
            GearKind::Icon => GearValue::Icon(buffer.read_s(table)?.unwrap_or_default()),
            GearKind::FontSize => GearValue::FontSize(buffer.read_i32()?),
            // 可见性齿轮不经过这里，调用点已经分支。
            GearKind::Display | GearKind::Display2 => GearValue::Text(String::new()),
        })
    }

    fn read_percent_value(
        &mut self,
        buffer: &mut ByteBuffer<'_>,
        page: Option<String>,
    ) -> Result<()> {
        let px = buffer.read_f32()?;
        let py = buffer.read_f32()?;
        if let Some(GearValue::Xy { px: x, py: y, .. }) = self.values.get_mut(&page) {
            *x = px;
            *y = py;
        }
        Ok(())
    }
}

/// 读取齿轮描述并挂到对象上。
///
/// 齿轮块位于实例数据的块 2。
pub(crate) fn setup_gears(
    store: &mut ObjectStore,
    owner: GObjectId,
    buffer: &mut ByteBuffer<'_>,
    table: &StringTable,
    begin_pos: usize,
) -> Result<()> {
    if !buffer.seek(begin_pos, 2)? {
        return Ok(());
    }

    let count = buffer.read_i16()?.max(0) as usize;
    for _ in 0..count {
        let next_pos = i32::from(buffer.read_i16()?);
        let end = buffer.resolve_relative(next_pos)?;

        let kind = GearKind::from_code(buffer.read_u8()?);
        let mut gear = Gear::new(kind);
        gear.controller_owner = store.object(owner).and_then(|o| o.parent());
        if let Some(value) = store.gear_snapshot(owner, kind) {
            gear.values.insert(None, value);
        }
        gear.setup(buffer, table)?;
        store.object_mut(owner).unwrap().gears.push(gear);

        buffer.set_position(end);
    }

    Ok(())
}

fn read_color(buffer: &mut ByteBuffer<'_>) -> Result<Color> {
    let r = buffer.read_u8()?;
    let g = buffer.read_u8()?;
    let b = buffer.read_u8()?;
    let a = buffer.read_u8()?;
    Ok(Color::from_rgba8(r, g, b, a))
}

impl GearValue {
    fn same_tween_target(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Xy { x, y, .. }, Self::Xy { x: ox, y: oy, .. }) => x == ox && y == oy,
            (
                Self::Look {
                    alpha, rotation, ..
                },
                Self::Look {
                    alpha: oa,
                    rotation: or,
                    ..
                },
            ) => alpha == oa && rotation == or,
            _ => self == other,
        }
    }

    fn copy_look_flags(&mut self, source: &Self) {
        if let (
            Self::Look {
                grayed, touchable, ..
            },
            Self::Look {
                grayed: sg,
                touchable: st,
                ..
            },
        ) = (self, source)
        {
            *grayed = *sg;
            *touchable = *st;
        }
    }

    /// 把取值套用到对象上。
    ///
    /// 这里只处理通用属性（坐标、尺寸、外观）；颜色、文本、图标与字号要改动
    /// 控件专有数据，由 [`Gear::apply`] 另行分派。
    pub fn apply(&self, store: &mut ObjectStore, owner: GObjectId, kind: GearKind) {
        store.with_gear_lock(owner, |store| self.apply_locked(store, owner, kind));
    }

    fn apply_locked(&self, store: &mut ObjectStore, owner: GObjectId, kind: GearKind) {
        match (kind, self) {
            (GearKind::Xy, Self::Xy { x, y, .. }) => store.set_xy(owner, *x, *y),
            (
                GearKind::Size,
                Self::Size {
                    width,
                    height,
                    scale_x,
                    scale_y,
                },
            ) => {
                store.set_size(owner, *width, *height);
                store.set_scale(owner, *scale_x, *scale_y);
            }
            (
                GearKind::Look,
                Self::Look {
                    alpha,
                    rotation,
                    grayed,
                    touchable,
                },
            ) => {
                store.set_alpha(owner, *alpha);
                store.set_rotation(owner, *rotation);
                store.set_grayed(owner, *grayed);
                store.set_touchable(owner, *touchable);
            }
            _ => {}
        }
    }

    /// 在两个同构取值之间插值。
    ///
    /// 类型不一致时无法插值，直接取终点。布尔量在过半时切换。
    pub fn lerp(&self, other: &Self, progress: f32) -> Self {
        match (self, other) {
            (
                Self::Xy {
                    x: sx,
                    y: sy,
                    px: spx,
                    py: spy,
                },
                Self::Xy {
                    x: ex,
                    y: ey,
                    px: epx,
                    py: epy,
                },
            ) => Self::Xy {
                x: sx + (ex - sx) * progress,
                y: sy + (ey - sy) * progress,
                px: spx + (epx - spx) * progress,
                py: spy + (epy - spy) * progress,
            },
            (
                Self::Size {
                    width: sw,
                    height: sh,
                    scale_x: ssx,
                    scale_y: ssy,
                },
                Self::Size {
                    width: ew,
                    height: eh,
                    scale_x: esx,
                    scale_y: esy,
                },
            ) => Self::Size {
                width: sw + (ew - sw) * progress,
                height: sh + (eh - sh) * progress,
                scale_x: ssx + (esx - ssx) * progress,
                scale_y: ssy + (esy - ssy) * progress,
            },
            (
                Self::Look {
                    alpha: sa,
                    rotation: sr,
                    grayed: sg,
                    touchable: st,
                },
                Self::Look {
                    alpha: ea,
                    rotation: er,
                    grayed: eg,
                    touchable: et,
                },
            ) => Self::Look {
                alpha: sa + (ea - sa) * progress,
                rotation: sr + (er - sr) * progress,
                grayed: if progress >= 0.5 { *eg } else { *sg },
                touchable: if progress >= 0.5 { *et } else { *st },
            },
            (
                Self::Color {
                    color: sc,
                    stroke_color: sst,
                },
                Self::Color {
                    color: ec,
                    stroke_color: est,
                },
            ) => Self::Color {
                color: crate::color::Color::new(
                    sc.r + (ec.r - sc.r) * progress,
                    sc.g + (ec.g - sc.g) * progress,
                    sc.b + (ec.b - sc.b) * progress,
                    sc.a + (ec.a - sc.a) * progress,
                ),
                stroke_color: crate::color::Color::new(
                    sst.r + (est.r - sst.r) * progress,
                    sst.g + (est.g - sst.g) * progress,
                    sst.b + (est.b - sst.b) * progress,
                    sst.a + (est.a - sst.a) * progress,
                ),
            },
            _ => other.clone(),
        }
    }
}

/// 读出对象当前的取值，作为补间的起点。
///
/// 只对支持插值的齿轮有意义；其余类型返回 `None`，调用方会退回立即设值。
fn snapshot_value(store: &ObjectStore, owner: GObjectId, kind: GearKind) -> Option<GearValue> {
    let object = store.object(owner)?;

    Some(match kind {
        GearKind::Xy => GearValue::Xy {
            x: object.x(),
            y: object.y(),
            px: 0.0,
            py: 0.0,
        },
        GearKind::Size => GearValue::Size {
            width: object.width(),
            height: object.height(),
            scale_x: object.scale_x(),
            scale_y: object.scale_y(),
        },
        GearKind::Look => GearValue::Look {
            alpha: object.alpha(),
            rotation: object.rotation(),
            grayed: object.grayed(),
            touchable: object.touchable(),
        },
        _ => return None,
    })
}

/// 两个取值是否同构。只有同构才谈得上在它们之间插值。
fn same_shape(start: &GearValue, end: &GearValue) -> bool {
    std::mem::discriminant(start) == std::mem::discriminant(end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gear_kinds_match_dispatch_table() {
        assert_eq!(GearKind::from_code(0), GearKind::Display);
        assert_eq!(GearKind::from_code(1), GearKind::Xy);
        assert_eq!(GearKind::from_code(3), GearKind::Look);
        assert_eq!(GearKind::from_code(8), GearKind::Display2);
        assert_eq!(GearKind::from_code(9), GearKind::FontSize);
        assert_eq!(GearKind::from_code(200), GearKind::Display);
    }

    #[test]
    fn new_gear_starts_empty() {
        let gear = Gear::new(GearKind::Xy);
        assert_eq!(gear.controller_index, -1);
        assert!(gear.values.is_empty());
        assert!(gear.tween.is_none());
        assert!(!gear.positions_in_percent);
    }
}
