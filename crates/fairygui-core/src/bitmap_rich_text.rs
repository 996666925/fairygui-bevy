//! Multi-command bitmap layout. Style boundaries participate in native wrapping.
use super::{BitmapTextGlyph, alphanumeric, is_punctuation, is_word, measure, round};
use crate::{
    geom::{Rect, Size},
    package::BitmapFontData,
    widget::TextFieldData,
};

#[cfg(test)]
#[path = "bitmap_rich_text_tests.rs"]
mod tests;

#[derive(Debug, Clone, PartialEq)]
pub enum BitmapElementKind {
    Text(String),
    Image(Size),
    LinkStart,
    LinkEnd,
}
#[derive(Debug, Clone, PartialEq)]
pub struct BitmapElement {
    pub kind: BitmapElementKind,
    pub font_size: i32,
}
#[derive(Debug, Clone, PartialEq)]
pub struct BitmapRichQuad {
    pub element: usize,
    pub glyph: BitmapTextGlyph,
}
#[derive(Debug, Clone, PartialEq)]
pub struct BitmapRichRun {
    pub element: usize,
    pub rect: Rect,
    pub font_size: f32,
    pub line_height: f32,
    pub line_y: f32,
}
#[derive(Debug, Clone, PartialEq)]
pub struct BitmapRichLayout {
    pub glyphs: Vec<BitmapRichQuad>,
    /// Image rectangles exclude the one-pixel horizontal margins reserved by Text.
    pub images: Vec<(usize, Rect)>,
    /// Command bounds for decorations, with their measured (not drawn) widths.
    pub runs: Vec<BitmapRichRun>,
    /// Native link areas, indexed by their opening LinkStart element.
    pub links: Vec<(usize, Rect)>,
    pub text_size: Size,
    pub object_size: Size,
    pub font_scale: f64,
}

#[derive(Debug, Clone)]
struct Cmd {
    element: usize,
    text: Option<Vec<u16>>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    font_size: f64,
    draw_size: f64,
    link_end: bool,
}
#[derive(Debug, Default)]
struct Line {
    cmds: Vec<Cmd>,
    width: f64,
    height: f64,
    x: f64,
    y: f64,
}
struct Engine<'a> {
    font: &'a BitmapFontData,
    elements: &'a [BitmapElement],
    lines: Vec<Line>,
    x: f64,
    char_height: f64,
    font_size: f64,
    element: usize,
    width: f64,
    scale: f64,
    leading: f64,
    wrap: bool,
    no_break: bool,
}
fn boundary(text: &[u16]) -> Option<usize> {
    if text.last().is_some_and(|&c| is_word(c)) {
        Some(text.iter().rposition(|&c| !is_word(c)).map_or(0, |i| i + 1))
    } else {
        None
    }
}
fn scaled_size(size: f64, scale: f64) -> f64 {
    let size = (size * scale).floor();
    if size == 0.0 { 1.0 } else { size }
}
impl Engine<'_> {
    fn finish(&mut self) {
        let line = self.lines.last_mut().unwrap();
        let height = line.cmds.iter().map(|c| c.height).fold(0.0, f64::max);
        for cmd in &mut line.cmds {
            cmd.y = ((height - cmd.height) * 0.5).floor();
        }
        line.width = round(line.cmds.iter().map(|c| c.width).sum());
        line.height = if height == 0.0 {
            self.char_height
        } else {
            height
        } + 1.0;
    }
    fn new_line(&mut self) {
        self.finish();
        let last = self.lines.last().unwrap();
        let y = last.y + last.height + self.leading;
        self.lines.push(Line {
            y,
            ..Default::default()
        });
        self.x = 0.0;
    }
    fn add_text(&mut self, text: &[u16], width: Option<f64>) {
        let width = width
            .filter(|&w| w != 0.0)
            .unwrap_or_else(|| measure(self.font, text, self.font_size));
        self.lines.last_mut().unwrap().cmds.push(Cmd {
            element: self.element,
            text: Some(text.to_vec()),
            x: self.x,
            y: 0.0,
            width,
            height: self.char_height,
            font_size: self.font_size,
            draw_size: f64::from(self.elements[self.element].font_size),
            link_end: false,
        });
        self.x += round(width);
    }
    fn add_object(&mut self, size: Size) {
        let width = f64::from(size.width);
        let margin = if width > 0.0 { 1.0 } else { 0.0 };
        self.lines.last_mut().unwrap().cmds.push(Cmd {
            element: self.element,
            text: None,
            x: self.x + margin,
            y: 0.0,
            width: width + margin * 2.0,
            height: f64::from(size.height),
            font_size: self.font_size,
            draw_size: f64::from(self.elements[self.element].font_size),
            link_end: false,
        });
        self.x += round(width + margin * 2.0);
    }
    // Native finalizes the old line before splitting/moving commands. Its saved
    // width/height therefore deliberately remain those from before the move.
    fn carry(&mut self, old: usize, at: usize, split: Option<usize>) {
        let start = if let Some(mut pos) = split {
            let cmd = &mut self.lines[old].cmds[at];
            let text = cmd.text.as_mut().unwrap();
            if text.get(pos).is_some_and(|c| (0xDC00..=0xDFFF).contains(c)) {
                pos = pos.saturating_sub(1);
            }
            if pos == 0 {
                at + 1
            } else {
                let suffix = text.split_off(pos);
                cmd.width = measure(self.font, text, cmd.font_size);
                let mut next = cmd.clone();
                next.text = Some(suffix);
                next.width = measure(self.font, next.text.as_ref().unwrap(), next.font_size);
                next.link_end = false;
                self.lines[old].cmds.insert(at + 1, next);
                at + 1
            }
        } else {
            at
        };
        let mut start = start;
        while start + 1 < self.lines[old].cmds.len() && self.lines[old].cmds[start].link_end {
            start += 1;
        }
        let moved: Vec<_> = self.lines[old].cmds.drain(start..).collect();
        for mut cmd in moved {
            cmd.x = self.x;
            cmd.y = 0.0;
            self.x += round(cmd.width);
            self.lines.last_mut().unwrap().cmds.push(cmd);
        }
    }
    fn wrap_text(&mut self, text: &[u16]) {
        let mut remaining = (self.width - self.x).max(0.0);
        let total = measure(self.font, text, self.font_size);
        if total <= remaining {
            self.add_text(text, Some(total));
            return;
        }
        let mut start = 0;
        let mut j = 0;
        let mut word_width = Some(0.0);
        let mut punc = false;
        while j < text.len() {
            let code = text[j];
            let count = if (0xD800..=0xDBFF).contains(&code)
                && text
                    .get(j + 1)
                    .is_some_and(|c| (0xDC00..=0xDFFF).contains(c))
            {
                2
            } else {
                1
            };
            let mut tw = Some(measure(self.font, &text[j..j + count], self.font_size));
            word_width = Some(word_width.unwrap_or(0.0) + tw.unwrap());
            if word_width.unwrap() <= remaining || (j == start && self.x == 0.0) {
                j += count;
                continue;
            }
            let mut end = j;
            word_width = Some(word_width.unwrap() - tw.unwrap());
            if self.no_break
                && (alphanumeric(code) || {
                    punc = is_punctuation(code);
                    punc
                })
            {
                let wb = if end > start {
                    boundary(&text[start..end])
                } else {
                    Some(0)
                };
                if let Some(wb) = wb.filter(|&b| b > 0) {
                    if wb as isize > (end - start) as isize - 20 {
                        j = start + wb;
                        end = j;
                        word_width = None;
                        tw = None;
                    }
                } else if wb.is_some() && !self.lines.last().unwrap().cmds.is_empty() {
                    let old = self.lines.len() - 1;
                    let mut total_len = end - start;
                    let mut moved = false;
                    for k in (0..self.lines[old].cmds.len()).rev() {
                        let cmd = &self.lines[old].cmds[k];
                        if cmd.width <= 0.0 {
                            continue;
                        }
                        let Some(previous) = &cmd.text else {
                            break;
                        };
                        let len = previous.len();
                        let b = boundary(previous);
                        let x = cmd.x;
                        if b.is_none() {
                            self.new_line();
                            if punc && total_len == 0 {
                                let pos = len.saturating_sub(1);
                                if pos > 0 {
                                    self.carry(old, k, Some(pos));
                                } else if x > 0.0 {
                                    self.carry(old, k, None);
                                }
                            } else {
                                self.carry(old, k + 1, None);
                            }
                            moved = true;
                            break;
                        } else if let Some(b) = b.filter(|&b| b > 0) {
                            if b as isize > len as isize - (20 - total_len as isize) {
                                self.new_line();
                                self.carry(old, k, Some(b));
                                moved = true;
                            }
                            break;
                        } else {
                            total_len += len;
                            if total_len >= 20 {
                                break;
                            }
                        }
                    }
                    if moved {
                        remaining = self.width - self.x;
                        if word_width.unwrap_or(0.0) + tw.unwrap_or(0.0) < remaining {
                            word_width = Some(word_width.unwrap_or(0.0) + tw.unwrap_or(0.0));
                            j += 1;
                            continue;
                        }
                    }
                } else if punc {
                    let back = if j >= 1 && (0xDC00..=0xDFFF).contains(&text[j - 1]) {
                        2
                    } else {
                        1
                    };
                    if j >= back && (j - back > start || self.x > 0.0) {
                        j -= back;
                        end = j;
                        word_width = None;
                        tw = None;
                    }
                }
            }
            if end > start {
                self.add_text(&text[start..end], word_width);
            }
            self.new_line();
            start = j;
            remaining = self.width;
            word_width = None;
            if let Some(tw) = tw {
                word_width = Some(tw);
                j += count - 1;
            } else if (0xD800..=0xDBFF).contains(&text[j]) {
                j += 1;
            }
            if word_width.is_none() && j < text.len() - 1 {
                word_width = Some(measure(self.font, &text[start..j + 1], self.font_size));
            }
            j += 1;
        }
        self.add_text(&text[start..], None);
    }
    fn run(mut self) -> Vec<Line> {
        for (i, element) in self.elements.iter().enumerate() {
            self.element = i;
            match &element.kind {
                BitmapElementKind::Text(text) => {
                    self.font_size = scaled_size(f64::from(element.font_size), self.scale);
                    self.char_height = if self.font.auto_scale_size {
                        round(
                            f64::from(self.font.line_height) * self.font_size
                                / f64::from(self.font.font_size),
                        )
                    } else {
                        f64::from(self.font.line_height)
                    };
                    let text: Vec<_> = text.encode_utf16().collect();
                    let lines: Vec<_> = text.split(|&c| c == 10).collect();
                    for (n, line) in lines.iter().enumerate() {
                        if !line.is_empty() {
                            if self.wrap {
                                self.wrap_text(line);
                            } else {
                                self.add_text(line, None);
                            }
                        }
                        if n + 1 < lines.len() {
                            self.new_line();
                        }
                    }
                }
                BitmapElementKind::LinkEnd => {
                    if let Some(cmd) = self.lines.last_mut().unwrap().cmds.last_mut() {
                        cmd.link_end = true;
                    }
                }
                kind => {
                    let size = if let BitmapElementKind::Image(size) = kind {
                        *size
                    } else {
                        Size::ZERO
                    };
                    if self.wrap
                        && size.width > 0.0
                        && self.width - self.x < f64::from(size.width) + 1.0
                        && self.x > 0.0
                    {
                        self.new_line();
                    }
                    self.add_object(size);
                }
            }
        }
        self.finish();
        self.lines
    }
}
fn dimensions(lines: &[Line]) -> (f64, f64) {
    let w = lines.iter().map(|l| l.width).fold(0.0, f64::max);
    let h = lines.last().map_or(0.0, |l| l.y + l.height);
    (
        w + if w > 0.0 { 4.0 } else { 0.0 },
        h + if h > 0.0 { 4.0 } else { 0.0 },
    )
}

/// Laya multi-command layout in logical pixels. Inline sizes stay fixed during
/// font shrink; style sizes affect measurement and drawing independently.
pub fn layout_bitmap_rich_text(
    font: &BitmapFontData,
    data: &TextFieldData,
    elements: &[BitmapElement],
    size: Size,
) -> BitmapRichLayout {
    let width_set = data.auto_size != 1;
    let height_set = !matches!(data.auto_size, 1 | 2);
    let width = if width_set {
        f64::from(size.width) - 4.0
    } else {
        f64::MAX
    };
    let height = if height_set {
        f64::from(size.height) - 4.0
    } else {
        f64::MAX
    };
    let wrap = !data.single_line && width_set;
    let run = |scale| {
        if elements.is_empty() {
            return Vec::new();
        }
        Engine {
            font,
            elements,
            lines: vec![Line::default()],
            x: 0.0,
            char_height: 0.0,
            font_size: 0.0,
            element: 0,
            width,
            scale,
            leading: (f64::from(data.leading) * scale).floor(),
            wrap: wrap || data.auto_size == 4,
            no_break: wrap,
        }
        .run()
    };
    let mut scale = 1.0;
    let mut lines = run(scale);
    let (mut tw, mut th) = dimensions(&lines);
    let fs = f64::from(data.font_size);
    if data.auto_size == 3 && !lines.is_empty() {
        if lines.len() > 1 && th > height {
            let (mut low, mut high) = (0.0, fs);
            scale = (height / th).max(0.0).sqrt();
            let mut current = (scale * fs).floor();
            for _ in 0..128 {
                lines = run(scale);
                (tw, th) = dimensions(&lines);
                if tw > width || th > height {
                    high = current;
                } else {
                    low = current;
                }
                if high - low > 1.0 || (high != low && current == high) {
                    current = low + (high - low) / 2.0;
                    scale = if fs == 0.0 { 0.0 } else { current / fs };
                } else {
                    break;
                }
            }
        } else if tw > width {
            scale = width / tw;
            lines = run(scale);
            (tw, th) = dimensions(&lines);
            if tw > width {
                scale = if fs == 0.0 {
                    0.0
                } else {
                    ((fs * scale).floor() - 1.0) / fs
                };
                lines = run(scale);
                (tw, th) = dimensions(&lines);
            }
        }
    } else if data.auto_size == 4 && (tw > width || th > height || (!wrap && lines.len() > 1)) {
        let count = if !wrap && lines.len() > 1 {
            Some(1)
        } else {
            lines
                .iter()
                .position(|l| l.y + l.height > height)
                .map(|i| i.max(1))
        };
        let deleted = count.is_some_and(|n| lines.len() > n);
        if deleted {
            lines.truncate(count.unwrap());
        }
        if let Some(line) = lines.last_mut() {
            let mut last_text = None;
            let mut stop = None;
            for i in 0..line.cmds.len() {
                if line.cmds[i].text.is_some() {
                    last_text = Some(line.cmds[i].clone());
                }
                let last = i + 1 == line.cmds.len();
                let cmd = &mut line.cmds[i];
                if (last && deleted) || cmd.x + cmd.width > width - 10.0 {
                    if let Some(text) = &mut cmd.text {
                        let space = cmd.x + cmd.width - width;
                        let remove = if space < 5.0 {
                            2
                        } else if space < 10.0 {
                            1
                        } else {
                            0
                        };
                        for _ in 0..remove {
                            if text.len() > usize::from(i == 0) {
                                if text.last().is_some_and(|c| (0xDC00..=0xDFFF).contains(c)) {
                                    text.pop();
                                }
                                text.pop();
                            }
                        }
                        text.push(8230);
                    } else {
                        if let Some(previous) = &last_text {
                            cmd.element = previous.element;
                            cmd.font_size = previous.font_size;
                            cmd.draw_size = previous.draw_size;
                            cmd.height = previous.height;
                        } else {
                            let last_size = elements.iter().rev().find_map(|e| {
                                matches!(e.kind, BitmapElementKind::Text(_)).then_some(e.font_size)
                            });
                            // Native has no measured font/height when every
                            // element is an image. Its NaN width is ignored by
                            // max-line measurement, while the ellipsis still
                            // draws with the field's default style size.
                            cmd.font_size =
                                last_size.map_or(f64::NAN, |s| scaled_size(f64::from(s), scale));
                            cmd.draw_size = fs;
                            cmd.height = if last_size.is_none() {
                                0.0
                            } else if font.auto_scale_size {
                                round(
                                    f64::from(font.line_height) * cmd.font_size
                                        / f64::from(font.font_size),
                                )
                            } else {
                                f64::from(font.line_height)
                            };
                        }
                        cmd.text = Some(vec![8230]);
                    }
                    cmd.width = measure(font, cmd.text.as_ref().unwrap(), cmd.font_size);
                    stop = Some(i + 1);
                    break;
                }
            }
            if let Some(n) = stop {
                line.cmds.truncate(n);
                let h = line.cmds.iter().map(|c| c.height).fold(0.0, f64::max);
                for c in &mut line.cmds {
                    c.y = ((h - c.height) * 0.5).floor();
                }
                line.height = h + 1.0;
                line.width = round(line.cmds.iter().map(|c| c.width).sum());
            }
        }
        (tw, th) = dimensions(&lines);
    }
    let object_size = Size::new(
        if data.auto_size == 1 {
            tw as f32
        } else {
            size.width
        },
        if matches!(data.auto_size, 1 | 2) {
            th as f32
        } else {
            size.height
        },
    );
    let vertical = if height_set && th < f64::from(object_size.height) {
        match data.vertical_align {
            1 => ((f64::from(object_size.height) - th) * 0.5).floor(),
            2 => f64::from(object_size.height) - th,
            _ => 0.0,
        }
    } else {
        0.0
    };
    let mut result = BitmapRichLayout {
        glyphs: Vec::new(),
        images: Vec::new(),
        runs: Vec::new(),
        links: Vec::new(),
        text_size: Size::new(tw as f32, th as f32),
        object_size,
        font_scale: scale,
    };
    let mut link = None;
    let mut link_start = 0.0;
    for line in &mut lines {
        let available = f64::from(object_size.width) - 4.0;
        if width_set {
            line.x = match data.align {
                1 => ((available - line.width) * 0.5).floor().max(0.0),
                2 => (available - line.width).max(0.0),
                _ => 0.0,
            };
        }
        let x = 2.0 + line.x;
        let y = 2.0 + vertical + line.y;
        for cmd in &line.cmds {
            if cmd.link_end
                && let Some(open) = link.take()
            {
                result.links.push((
                    open,
                    Rect::new(
                        link_start as f32,
                        y as f32,
                        (x + cmd.x + cmd.width - link_start) as f32,
                        line.height as f32,
                    ),
                ));
            }
            let rect = Rect::new(
                (x + cmd.x) as f32,
                (y + cmd.y) as f32,
                cmd.width as f32,
                cmd.height as f32,
            );
            result.runs.push(BitmapRichRun {
                element: cmd.element,
                rect,
                font_size: cmd.font_size as f32,
                line_height: line.height as f32,
                line_y: y as f32,
            });
            if let Some(text) = &cmd.text {
                let draw_scale = ((if font.auto_scale_size {
                    cmd.draw_size
                } else {
                    f64::from(font.font_size)
                }) * scale)
                    .floor()
                    / f64::from(font.font_size);
                let mut tx = 0.0;
                for &code in text {
                    if let Some(g) = font.glyph(u32::from(code)) {
                        if g.texture.is_some() {
                            result.glyphs.push(BitmapRichQuad {
                                element: cmd.element,
                                glyph: BitmapTextGlyph {
                                    char_code: u32::from(code),
                                    x: (x + cmd.x + tx + f64::from(g.x) * draw_scale) as f32,
                                    y: (y + cmd.y + f64::from(g.y) * draw_scale) as f32,
                                    width: (f64::from(g.width) * draw_scale) as f32,
                                    height: (f64::from(g.height) * draw_scale) as f32,
                                },
                            });
                        }
                        tx += round(f64::from(g.advance) * draw_scale);
                    }
                }
            } else {
                match elements[cmd.element].kind {
                    BitmapElementKind::LinkStart => {
                        link = Some(cmd.element);
                        link_start = x + cmd.x;
                    }
                    BitmapElementKind::Image(size) => result.images.push((
                        cmd.element,
                        Rect::new(rect.x, rect.y, size.width, size.height),
                    )),
                    _ => {}
                }
            }
        }
        if let Some(open) = link {
            result.links.push((
                open,
                Rect::new(
                    link_start as f32,
                    y as f32,
                    ((f64::from(object_size.width) - 2.0).max(0.0) - link_start) as f32,
                    line.height as f32,
                ),
            ));
            link_start = 2.0;
        }
    }
    result
}
