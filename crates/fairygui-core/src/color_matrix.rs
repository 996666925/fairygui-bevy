//! 颜色矩阵。
//!
//! 提供亮度、对比度、饱和度与色相四种调整，并组合成一个 4×5 矩阵。
//! 做法与图形学惯例一致：把颜色看作列向量，前四列做线性变换，第五列是偏移。
//!
//! `ColorFilterParams` 遵循 FairyGUI 包格式：四项均以 0 为默认值，范围 -1..1。
//! `ColorMatrix` 的独立构造函数仍使用常规倍数/角度，勿与包参数混用。

use crate::color::Color;

/// 亮度权重（Rec. 709）。
const LUM_R: f32 = 0.2125;
const LUM_G: f32 = 0.7154;
const LUM_B: f32 = 0.0721;

/// 色相旋转使用的权重（Graphics Gems 的惯例取值）。
const HUE_R: f32 = 0.213;
const HUE_G: f32 = 0.715;
const HUE_B: f32 = 0.072;

/// 颜色滤镜的四个参数。
///
/// 描述里紧接着「是否有滤镜」的标志之后按顺序给出这四个浮点，
/// 顺序容易记错：**亮度、对比度、饱和度、色相**。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ColorFilterParams {
    /// 亮度加法偏移，0 表示不变，1 表示增加一个完整通道范围。
    pub brightness: f32,
    /// 对比度变化量，0 表示不变，-1 表示常量中灰。
    pub contrast: f32,
    /// 饱和度变化量，0 表示不变，-1 表示去饱和。
    pub saturation: f32,
    /// 色相旋转的半圈比例，0 表示不变，1 表示 180 度。
    pub hue: f32,
}

impl ColorFilterParams {
    /// 是否为「不产生任何变化」的参数。
    ///
    /// 仅判断矩阵是否恒等。显式滤镜仍可建立离屏隔离，不能据此删除隔离边界。
    pub fn is_noop(&self) -> bool {
        [self.brightness, self.contrast, self.saturation, self.hue]
            .into_iter()
            .all(|v| v == 0.0 || v.is_nan())
    }

    /// 转成颜色矩阵。
    pub fn to_matrix(self) -> ColorMatrix {
        native_matrix(self)
    }

    /// 滤镜作用在白色上的结果。
    ///
    /// 仅供不能执行矩阵的后端降级使用。乘算无法表达偏移或通道混合。
    pub fn tint(self) -> crate::color::Color {
        self.to_matrix().apply(crate::color::Color::WHITE)
    }
}

/// FairyGUI-Laya ColorMatrix.adjustColor: hue, contrast, brightness, saturation.
/// JS Number intermediates and 0..255 offsets are retained until final conversion.
fn native_matrix(params: ColorFilterParams) -> ColorMatrix {
    let clean = |v: f32| {
        if v.is_nan() {
            0.0
        } else {
            f64::from(v).clamp(-1.0, 1.0)
        }
    };
    let (r, g, b) = (0.299, 0.587, 0.114);
    let angle = clean(params.hue) * std::f64::consts::PI;
    let (sin, cos) = angle.sin_cos();
    let mut matrix = [
        r + cos * (1.0 - r) - sin * r,
        g - cos * g - sin * g,
        b - cos * b + sin * (1.0 - b),
        0.0,
        0.0,
        r - cos * r + sin * 0.143,
        g + cos * (1.0 - g) + sin * 0.14,
        b - cos * b - sin * 0.283,
        0.0,
        0.0,
        r - cos * r - sin * (1.0 - r),
        g - cos * g + sin * g,
        b + cos * (1.0 - b) + sin * b,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
    ];
    let mut multiply = |other: [f64; 20]| {
        let mut out = [0.0; 20];
        for row in 0..4 {
            for col in 0..5 {
                out[row * 5 + col] = (0..4)
                    .map(|k| other[row * 5 + k] * matrix[k * 5 + col])
                    .sum::<f64>()
                    + if col == 4 { other[row * 5 + 4] } else { 0.0 };
            }
        }
        matrix = out;
    };
    let scale = clean(params.contrast) + 1.0;
    let offset = 128.0 * (1.0 - scale);
    multiply([
        scale, 0.0, 0.0, 0.0, offset, 0.0, scale, 0.0, 0.0, offset, 0.0, 0.0, scale, 0.0, offset,
        0.0, 0.0, 0.0, 1.0, 0.0,
    ]);
    let offset = clean(params.brightness) * 255.0;
    multiply([
        1.0, 0.0, 0.0, 0.0, offset, 0.0, 1.0, 0.0, 0.0, offset, 0.0, 0.0, 1.0, 0.0, offset, 0.0,
        0.0, 0.0, 1.0, 0.0,
    ]);
    let scale = clean(params.saturation) + 1.0;
    let (ir, ig, ib) = ((1.0 - scale) * r, (1.0 - scale) * g, (1.0 - scale) * b);
    multiply([
        ir + scale,
        ig,
        ib,
        0.0,
        0.0,
        ir,
        ig + scale,
        ib,
        0.0,
        0.0,
        ir,
        ig,
        ib + scale,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
    ]);
    ColorMatrix {
        values: std::array::from_fn(|i| (matrix[i] / if i % 5 == 4 { 255.0 } else { 1.0 }) as f32),
    }
}

/// 4×5 颜色矩阵。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorMatrix {
    /// 按行存储的 20 个系数。
    pub values: [f32; 20],
}

impl Default for ColorMatrix {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl ColorMatrix {
    /// 单位矩阵：不改变颜色。
    pub const IDENTITY: Self = Self {
        values: [
            1.0, 0.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 0.0, 1.0, 0.0,
        ],
    };

    /// Laya ColorFilter.gray(), distinct from FairyGUI saturation = -1.
    /// The legacy ToolSet path temporarily replaces any local color filter.
    pub const LAYA_GRAY: Self = Self {
        values: [
            0.3086, 0.6094, 0.082, 0.0, 0.0, 0.3086, 0.6094, 0.082, 0.0, 0.0, 0.3086, 0.6094,
            0.082, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0,
        ],
    };

    /// 依次叠加亮度、对比度、饱和度与色相。
    pub fn from_params(brightness: f32, contrast: f32, saturation: f32, hue: f32) -> Self {
        Self::brightness(brightness)
            .then(&Self::contrast(contrast))
            .then(&Self::saturation(saturation))
            .then(&Self::hue(hue))
    }

    /// 先应用 `self`，再应用 `other`。
    pub fn then(&self, other: &Self) -> Self {
        let mut out = [0.0f32; 20];

        for row in 0..4 {
            for col in 0..5 {
                let mut sum = 0.0;
                for k in 0..4 {
                    sum += other.values[row * 5 + k] * self.values[k * 5 + col];
                }
                // 第五列还要叠加对方的偏移项。
                if col == 4 {
                    sum += other.values[row * 5 + 4];
                }
                out[row * 5 + col] = sum;
            }
        }

        Self { values: out }
    }

    /// 对颜色应用矩阵。结果会被夹在合法范围内。
    pub fn apply(&self, color: Color) -> Color {
        let (r, g, b, a) = (color.r, color.g, color.b, color.a);
        let v = &self.values;

        Color::new(
            (v[0] * r + v[1] * g + v[2] * b + v[3] * a + v[4]).clamp(0.0, 1.0),
            (v[5] * r + v[6] * g + v[7] * b + v[8] * a + v[9]).clamp(0.0, 1.0),
            (v[10] * r + v[11] * g + v[12] * b + v[13] * a + v[14]).clamp(0.0, 1.0),
            (v[15] * r + v[16] * g + v[17] * b + v[18] * a + v[19]).clamp(0.0, 1.0),
        )
    }

    /// 是否为单位矩阵（不产生任何变化）。
    pub fn is_identity(&self) -> bool {
        self.values == Self::IDENTITY.values
    }

    /// 亮度调整。`1.0` 表示不变。
    pub fn brightness(value: f32) -> Self {
        let mut matrix = Self::IDENTITY;
        matrix.values[0] = value;
        matrix.values[6] = value;
        matrix.values[12] = value;
        matrix
    }

    /// 对比度调整。`1.0` 表示不变，偏移取 `(1 - value) / 2` 使中灰保持不动。
    pub fn contrast(value: f32) -> Self {
        let offset = (1.0 - value) * 0.5;

        let mut matrix = Self::IDENTITY;
        matrix.values[0] = value;
        matrix.values[6] = value;
        matrix.values[12] = value;
        matrix.values[4] = offset;
        matrix.values[9] = offset;
        matrix.values[14] = offset;
        matrix
    }

    /// 饱和度调整。`0` 为灰度，`1` 为不变，大于 1 会更鲜艳。
    pub fn saturation(value: f32) -> Self {
        let inverse = 1.0 - value;

        Self {
            values: [
                LUM_R * inverse + value,
                LUM_G * inverse,
                LUM_B * inverse,
                0.0,
                0.0,
                LUM_R * inverse,
                LUM_G * inverse + value,
                LUM_B * inverse,
                0.0,
                0.0,
                LUM_R * inverse,
                LUM_G * inverse,
                LUM_B * inverse + value,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
                0.0,
            ],
        }
    }

    /// 色相旋转，单位为角度。
    pub fn hue(degrees: f32) -> Self {
        let radians = degrees.to_radians();
        let cos = radians.cos();
        let sin = radians.sin();

        let (r, g, b) = (HUE_R, HUE_G, HUE_B);

        Self {
            values: [
                r + cos * (1.0 - r) - sin * r,
                g - cos * g - sin * g,
                b - cos * b + sin * (1.0 - b),
                0.0,
                0.0,
                r - cos * r + sin * 0.143,
                g + cos * (1.0 - g) + sin * 0.140,
                b - cos * b - sin * 0.283,
                0.0,
                0.0,
                r - cos * r - sin * (1.0 - r),
                g - cos * g + sin * g,
                b + cos * (1.0 - b) + sin * b,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
                0.0,
            ],
        }
    }

    /// 灰度化，等价于把饱和度设为 0。
    pub fn grayscale() -> Self {
        Self::saturation(0.0)
    }
}

#[cfg(test)]
#[path = "color_matrix_tests.rs"]
mod native_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn red() -> Color {
        Color::new(1.0, 0.0, 0.0, 1.0)
    }

    #[test]
    fn identity_changes_nothing() {
        let color = Color::new(0.3, 0.6, 0.9, 1.0);
        assert_eq!(ColorMatrix::IDENTITY.apply(color), color);
        assert!(ColorMatrix::IDENTITY.is_identity());
    }

    #[test]
    fn brightness_scales_channels() {
        let matrix = ColorMatrix::brightness(0.5);
        let result = matrix.apply(Color::new(1.0, 0.8, 0.4, 1.0));

        assert!((result.r - 0.5).abs() < 1e-6);
        assert!((result.g - 0.4).abs() < 1e-6);
        assert!((result.b - 0.2).abs() < 1e-6);
        assert_eq!(result.a, 1.0, "不应影响透明度");
    }

    #[test]
    fn contrast_keeps_mid_gray_in_place() {
        let matrix = ColorMatrix::contrast(2.0);
        let mid = Color::new(0.5, 0.5, 0.5, 1.0);
        let result = matrix.apply(mid);

        assert!(
            (result.r - 0.5).abs() < 1e-6,
            "中灰在对比度调整下应当保持不动，实际 {}",
            result.r
        );

        // 亮的更亮、暗的更暗。
        assert!(matrix.apply(Color::new(0.8, 0.8, 0.8, 1.0)).r > 0.8);
        assert!(matrix.apply(Color::new(0.2, 0.2, 0.2, 1.0)).r < 0.2);
    }

    #[test]
    fn saturation_zero_produces_gray() {
        let matrix = ColorMatrix::saturation(0.0);
        let result = matrix.apply(red());

        assert!(
            (result.r - result.g).abs() < 1e-4 && (result.g - result.b).abs() < 1e-4,
            "灰度化后三通道应当相等，实际 {result:?}"
        );
    }

    #[test]
    fn saturation_one_is_identity() {
        let matrix = ColorMatrix::saturation(1.0);
        let color = Color::new(0.2, 0.7, 0.4, 1.0);
        let result = matrix.apply(color);

        assert!((result.r - color.r).abs() < 1e-5);
        assert!((result.g - color.g).abs() < 1e-5);
        assert!((result.b - color.b).abs() < 1e-5);
    }

    #[test]
    fn hue_full_turn_returns_to_origin() {
        let matrix = ColorMatrix::hue(360.0);
        let color = Color::new(0.9, 0.3, 0.1, 1.0);
        let result = matrix.apply(color);

        assert!((result.r - color.r).abs() < 1e-3, "实际 {result:?}");
        assert!((result.g - color.g).abs() < 1e-3, "实际 {result:?}");
        assert!((result.b - color.b).abs() < 1e-3, "实际 {result:?}");
    }

    #[test]
    fn hue_rotation_changes_channels() {
        let matrix = ColorMatrix::hue(120.0);
        let result = matrix.apply(red());

        // 旋转后红色应当被转移到其它通道，且不再以红为主。
        assert!(
            result.g > 0.3 && result.r < 0.8,
            "色相旋转应当把能量移到别的通道，实际 {result:?}"
        );
    }

    #[test]
    fn alpha_is_preserved_by_standard_adjustments() {
        let matrix = ColorMatrix::from_params(1.2, 1.1, 0.8, 30.0);
        let result = matrix.apply(Color::new(0.5, 0.5, 0.5, 0.42));

        assert!(
            (result.a - 0.42).abs() < 1e-5,
            "常规调整不应改动透明度，实际 {}",
            result.a
        );
    }

    #[test]
    fn composition_order_matters_and_is_applied_in_sequence() {
        // 先变亮再变暗应当相互抵消。
        let combined = ColorMatrix::brightness(2.0).then(&ColorMatrix::brightness(0.5));
        let color = Color::new(0.4, 0.4, 0.4, 1.0);
        let result = combined.apply(color);

        assert!((result.r - 0.4).abs() < 1e-6, "实际 {}", result.r);
    }

    #[test]
    fn results_are_clamped() {
        let matrix = ColorMatrix::brightness(10.0);
        let result = matrix.apply(Color::new(1.0, 1.0, 1.0, 1.0));
        assert_eq!(result.r, 1.0);

        let dark = ColorMatrix::brightness(-5.0).apply(Color::new(0.5, 0.5, 0.5, 1.0));
        assert_eq!(dark.r, 0.0);
    }

    #[test]
    fn default_is_identity() {
        assert!(ColorMatrix::default().is_identity());
        assert!(ColorMatrix::grayscale().apply(red()).r == ColorMatrix::grayscale().apply(red()).g);
    }
}
