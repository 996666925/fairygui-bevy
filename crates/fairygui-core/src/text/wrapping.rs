//! Laya Text command wrapping with caller-provided font measurements.
use crate::{
    bitmap_text::{alphanumeric, is_punctuation, is_word, round},
    geom::Size,
};
#[cfg(test)]
#[path = "wrapping_tests.rs"]
mod tests;

#[derive(Debug, Clone, PartialEq)]
pub enum WrapKind {
    Text(String),
    Object(Size),
    LinkEnd,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WrapElement {
    pub kind: WrapKind,
    pub font_size: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct WrapOptions {
    pub width: f64,
    pub leading: f64,
    pub wrap: bool,
    pub no_break_word: bool,
    /// Enable the ordinary-font prefix estimate from Laya Text.wrapText.
    pub raster: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WrappedCommand {
    pub element: usize,
    pub text: Option<Vec<u16>>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub font_size: f64,
    pub link_end: bool,
}
#[derive(Debug, Default, Clone, PartialEq)]
pub struct WrappedLine {
    pub cmds: Vec<WrappedCommand>,
    pub width: f64,
    pub height: f64,
    pub x: f64,
    pub y: f64,
}
struct Engine<'a, M> {
    measure: M,
    elements: &'a [WrapElement],
    lines: Vec<WrappedLine>,
    x: f64,
    char_height: f64,
    font_size: f64,
    element: usize,
    width: f64,
    raster: bool,
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
impl<M: FnMut(usize, &[u16], f64) -> f64> Engine<'_, M> {
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
        self.lines.push(WrappedLine {
            y,
            ..Default::default()
        });
        self.x = 0.0;
    }
    fn add_text(&mut self, text: &[u16], width: Option<f64>) {
        let width = width
            .filter(|&w| w != 0.0)
            .unwrap_or_else(|| (self.measure)(self.element, text, self.font_size));
        self.lines.last_mut().unwrap().cmds.push(WrappedCommand {
            element: self.element,
            text: Some(text.to_vec()),
            x: self.x,
            y: 0.0,
            width,
            height: self.char_height,
            font_size: self.font_size,
            link_end: false,
        });
        self.x += round(width);
    }
    fn add_object(&mut self, size: Size) {
        let width = f64::from(size.width);
        let margin = if width > 0.0 { 1.0 } else { 0.0 };
        self.lines.last_mut().unwrap().cmds.push(WrappedCommand {
            element: self.element,
            text: None,
            x: self.x + margin,
            y: 0.0,
            width: width + margin * 2.0,
            height: f64::from(size.height),
            font_size: self.font_size,
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
                cmd.width = (self.measure)(cmd.element, text, cmd.font_size);
                let mut next = cmd.clone();
                next.text = Some(suffix);
                next.width =
                    (self.measure)(next.element, next.text.as_ref().unwrap(), next.font_size);
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
        let total = (self.measure)(self.element, text, self.font_size);
        if total <= remaining {
            self.add_text(text, Some(total));
            return;
        }
        let mut start = 0;
        let emoji = text.windows(2).any(|pair| {
            (0xD800..=0xDBFF).contains(&pair[0]) && (0xDC00..=0xDFFF).contains(&pair[1])
        });
        let maybe = if self.raster && !emoji {
            (remaining / self.font_size).floor().max(0.) as usize
        } else {
            0
        };
        let mut j = maybe;
        let mut word_width = Some(if maybe > 0 {
            (self.measure)(self.element, &text[..maybe.min(text.len())], self.font_size)
        } else {
            0.
        });
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
            let mut tw = Some((self.measure)(
                self.element,
                &text[j..j + count],
                self.font_size,
            ));
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
            if maybe > 1 {
                j = j.saturating_add(maybe - 1);
            } else if let Some(tw) = tw {
                word_width = Some(tw);
                j += count - 1;
            } else if (0xD800..=0xDBFF).contains(&text[j]) {
                j += 1;
            }
            if word_width.is_none() && j < text.len() - 1 {
                word_width = Some((self.measure)(
                    self.element,
                    &text[start..j + 1],
                    self.font_size,
                ));
            }
            j += 1;
        }
        self.add_text(&text[start..], None);
    }
    fn run(mut self) -> Vec<WrappedLine> {
        for (i, element) in self.elements.iter().enumerate() {
            self.element = i;
            match &element.kind {
                WrapKind::Text(text) => {
                    self.font_size = element.font_size;
                    self.char_height = element.height;
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
                WrapKind::LinkEnd => {
                    if let Some(cmd) = self.lines.last_mut().unwrap().cmds.last_mut() {
                        cmd.link_end = true;
                    }
                }
                kind => {
                    let size = if let WrapKind::Object(size) = kind {
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

/// Measurement receives the source element index, UTF-16 text and effective
/// font size. Returned line metrics include native pre-move finalization.
pub fn wrap_commands(
    elements: &[WrapElement],
    options: WrapOptions,
    measure: impl FnMut(usize, &[u16], f64) -> f64,
) -> Vec<WrappedLine> {
    if elements.is_empty() {
        return Vec::new();
    }
    Engine {
        measure,
        elements,
        lines: vec![WrappedLine::default()],
        x: 0.,
        char_height: 0.,
        font_size: 0.,
        element: 0,
        width: options.width,
        raster: options.raster,
        leading: options.leading,
        wrap: options.wrap,
        no_break: options.no_break_word,
    }
    .run()
}
