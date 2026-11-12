//! 颜色。
//!
//! 核心层用浮点分量表示颜色，以便直接与渲染层对接；
//! 描述文件里的颜色是四个 8 位分量（红、绿、蓝、透明度，顺序固定）。

/// 颜色，各分量为 0~1。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    /// 白色，也是「不做任何着色」的默认值。
    pub const WHITE: Self = Self::new(1.0, 1.0, 1.0, 1.0);

    /// 完全透明。
    pub const TRANSPARENT: Self = Self::new(0.0, 0.0, 0.0, 0.0);

    /// 由浮点分量构造。
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// 由 8 位分量构造。
    pub fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::new(
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
            f32::from(a) / 255.0,
        )
    }

    /// 是否为白色（即不产生任何着色效果）。
    pub fn is_white(&self) -> bool {
        self.r == 1.0 && self.g == 1.0 && self.b == 1.0
    }

    /// 转为 32 位 `0xAARRGGBB`。
    pub fn to_argb(&self) -> u32 {
        let channel = |value: f32| -> u32 { (value.clamp(0.0, 1.0) * 255.0).round() as u32 & 0xFF };
        (channel(self.a) << 24) | (channel(self.r) << 16) | (channel(self.g) << 8) | channel(self.b)
    }

    /// 由 32 位 `0xAARRGGBB` 构造。
    pub fn from_argb(value: u32) -> Self {
        Self::from_rgba8(
            ((value >> 16) & 0xFF) as u8,
            ((value >> 8) & 0xFF) as u8,
            (value & 0xFF) as u8,
            ((value >> 24) & 0xFF) as u8,
        )
    }

    /// 按比例与另一颜色相乘（用于父级不透明度叠加）。
    pub fn multiply_alpha(&self, alpha: f32) -> Self {
        Self::new(self.r, self.g, self.b, self.a * alpha)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_argb() {
        let color = Color::from_rgba8(0x12, 0x34, 0x56, 0x78);
        assert_eq!(color.to_argb(), 0x78123456);
        assert_eq!(Color::from_argb(0x78123456), color);
    }

    #[test]
    fn white_and_transparent_are_recognised() {
        assert!(Color::WHITE.is_white());
        assert!(!Color::from_rgba8(1, 2, 3, 255).is_white());
        assert_eq!(Color::TRANSPARENT.a, 0.0);
    }

    #[test]
    fn multiply_alpha_only_touches_alpha() {
        let color = Color::new(0.5, 0.25, 0.75, 0.8).multiply_alpha(0.5);
        assert_eq!((color.r, color.g, color.b), (0.5, 0.25, 0.75));
        assert!((color.a - 0.4).abs() < 1e-6);
    }
}
