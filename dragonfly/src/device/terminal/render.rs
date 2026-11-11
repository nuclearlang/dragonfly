//! Drawing a frame of the device in a terminal (pure functions: no I/O).
//! - `Render::Kitty`: the PNG as it comes, with the kitty graphics protocol (kitty, Ghostty, WezTerm).
//! - `Render::Sixel`: sixel graphics (foot, Konsole, Windows Terminal, iTerm2, mlterm, xterm -ti vt340).
//! - `Render::Blocks`: two pixels per cell with `▀` (any terminal with Unicode).
//! - `Render::Text`: blocks, with the app's texts written as letters on top (readable at any size).
//!
//! Colors in truecolor, 256 or 16 colors, by what the terminal supports.
use super::super::backend::TextRun;
use base64::Engine;
use std::fmt::Write;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Render {
    Kitty,
    Sixel,
    Blocks,
    Text,
}

impl Render {
    pub fn name(self) -> &'static str {
        match self {
            Render::Kitty => "kitty graphics",
            Render::Sixel => "sixel",
            Render::Blocks => "blocks",
            Render::Text => "blocks + text",
        }
    }

    pub fn parse(s: &str) -> Option<Render> {
        Some(match s {
            "kitty" => Render::Kitty,
            "sixel" => Render::Sixel,
            "blocks" => Render::Blocks,
            "text" => Render::Text,
            _ => return None,
        })
    }

    pub fn graphics(self) -> bool {
        matches!(self, Render::Kitty | Render::Sixel)
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Colors {
    True,
    Ansi256,
    Ansi16,
}

pub type Rgb = [u8; 3];

/// Where the screen of the device goes in the terminal, in cells, and the frame size to ask for.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Area {
    pub col: u16,
    pub row: u16,
    pub cols: u16,
    pub rows: u16,
}

/// The biggest area with the device's aspect inside `avail` (cells), centered. `cell` = pixel size of a cell.
pub fn fit(css: (u32, u32), avail: Area, cell: (f64, f64)) -> Area {
    let (w, h) = (css.0 as f64, css.1 as f64);
    let (cw, ch) = cell;
    let mut rows = avail.rows as f64;
    let mut cols = (rows * ch * w / (h * cw)).round();
    if cols > avail.cols as f64 {
        cols = avail.cols as f64;
        rows = (cols * cw * h / (w * ch)).round();
    }
    let (cols, rows) = (cols.max(1.0) as u16, rows.max(1.0) as u16);
    Area { col: avail.col + (avail.cols - cols) / 2, row: avail.row + (avail.rows - rows) / 2, cols, rows }
}

/// Size of the frame to ask the browser for: pixels for graphics, two pixels per cell (twice, to average) for
/// blocks.
pub fn frame_size(render: Render, area: Area, cell: (f64, f64)) -> (u32, u32) {
    if render.graphics() {
        ((area.cols as f64 * cell.0) as u32, (area.rows as f64 * cell.1) as u32)
    } else {
        (area.cols as u32 * 2, area.rows as u32 * 4)
    }
}

/// An RGB image.
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub px: Vec<Rgb>,
}

pub fn decode_png(bytes: &[u8]) -> Option<Image> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let step = info.color_type.samples();
    let (width, height) = (info.width as usize, info.height as usize);
    let px = buf[..info.buffer_size()]
        .chunks(step)
        .map(|c| match step {
            1 | 2 => [c[0]; 3],
            _ => [c[0], c[1], c[2]],
        })
        .collect();
    Some(Image { width, height, px })
}

impl Image {
    /// Resamples to `w` x `h` averaging the source pixels of each target pixel (box filter).
    pub fn resize(&self, w: usize, h: usize) -> Image {
        let mut px = Vec::with_capacity(w * h);
        for y in 0..h {
            let (y0, y1) = (y * self.height / h, ((y + 1) * self.height / h).max(y * self.height / h + 1));
            for x in 0..w {
                let (x0, x1) = (x * self.width / w, ((x + 1) * self.width / w).max(x * self.width / w + 1));
                let mut sum = [0u32; 3];
                let mut n = 0;
                for sy in y0..y1.min(self.height) {
                    for sx in x0..x1.min(self.width) {
                        let p = self.px[sy * self.width + sx];
                        sum[0] += p[0] as u32;
                        sum[1] += p[1] as u32;
                        sum[2] += p[2] as u32;
                        n += 1;
                    }
                }
                let n = n.max(1);
                px.push([(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8]);
            }
        }
        Image { width: w, height: h, px }
    }
}

/// One terminal cell.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Cell {
    pub ch: char,
    pub fg: Rgb,
    pub bg: Rgb,
    pub bold: bool,
}

/// The cells of the screen: half blocks, and with `texts` the app's texts written on top.
pub fn cells(img: &Image, area: Area, css: (u32, u32), texts: &[TextRun]) -> Vec<Cell> {
    let (cols, rows) = (area.cols as usize, area.rows as usize);
    let img = img.resize(cols, rows * 2);
    let mut out: Vec<Cell> = (0..cols * rows)
        .map(|i| {
            let (c, r) = (i % cols, i / cols);
            let (top, bottom) = (img.px[2 * r * cols + c], img.px[(2 * r + 1) * cols + c]);
            if top == bottom {
                Cell { ch: ' ', fg: top, bg: bottom, bold: false }
            } else {
                Cell { ch: '▀', fg: top, bg: bottom, bold: false }
            }
        })
        .collect();
    let (sx, sy) = (cols as f64 / css.0 as f64, rows as f64 / css.1 as f64);
    // where each text goes: the rows its box covers (its blurred letters), the row of its middle, its columns
    struct Place<'a> {
        t: &'a TextRun,
        rows: std::ops::RangeInclusive<usize>,
        row: usize,
        start: usize,
        end: usize,
        back: Rgb,
    }
    let at =
        |out: &[Cell], r: usize, c: usize| (c < cols && r < rows).then(|| [out[r * cols + c].fg, out[r * cols + c].bg]);
    let mut places = vec![];
    for t in texts {
        let row = ((t.y + t.h / 2.0) * sy).floor();
        if row < 0.0 || row >= rows as f64 || t.x * sx >= cols as f64 || t.x + t.w < 0.0 {
            continue;
        }
        let row = row as usize;
        let top = ((t.y + 1.0) * sy).floor().max(0.0) as usize;
        let bottom = (((t.y + t.h - 1.0) * sy).floor().max(0.0) as usize).clamp(row, rows - 1);
        let start = (t.x * sx).round().max(0.0) as usize;
        let len = t.text.chars().count();
        let end = ((t.x + t.w) * sx).round().max((start + len) as f64).min(cols as f64) as usize;
        // the background: the most common color around the box (inside it, the blurred letters weigh too much)
        let mut around = vec![];
        for c in start.saturating_sub(1)..=end {
            for r in [top.wrapping_sub(1), bottom + 1] {
                around.extend(at(&out, r, c).into_iter().flatten());
            }
        }
        for r in top..=bottom {
            around.extend(at(&out, r, start.wrapping_sub(1)).into_iter().flatten());
            around.extend(at(&out, r, end).into_iter().flatten());
        }
        let back = if around.is_empty() {
            most_common(out[row * cols + start..row * cols + end].iter().flat_map(|c| [c.fg, c.bg]))
        } else {
            most_common(around.into_iter())
        };
        places.push(Place { t, rows: top.min(row)..=bottom, row, start, end, back });
    }
    // first every blurred text goes away, then the letters (a text never erases another one)
    for p in &places {
        for r in p.rows.clone() {
            for cell in &mut out[r * cols + p.start..r * cols + p.end] {
                *cell = Cell { ch: ' ', fg: p.back, bg: p.back, bold: false };
            }
        }
    }
    for p in &places {
        let fg = readable(p.t.color.unwrap_or([0, 0, 0]), p.back);
        for (i, ch) in p.t.text.chars().take(p.end - p.start).enumerate() {
            out[p.row * cols + p.start + i] = Cell { ch, fg, bg: p.back, bold: p.t.bold };
        }
    }
    out
}

fn most_common(colors: impl Iterator<Item = Rgb>) -> Rgb {
    let mut counts: Vec<(Rgb, u32)> = vec![];
    for c in colors {
        match counts.iter_mut().find(|(k, _)| close(*k, c)) {
            Some((_, n)) => *n += 1,
            None => counts.push((c, 1)),
        }
    }
    counts.into_iter().max_by_key(|(_, n)| *n).map_or([255; 3], |(c, _)| c)
}

fn close(a: Rgb, b: Rgb) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) < 12)
}

fn luma(c: Rgb) -> f64 {
    0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64
}

/// The text's own color, or black/white when it would not read on that background.
fn readable(fg: Rgb, bg: Rgb) -> Rgb {
    if (luma(fg) - luma(bg)).abs() >= 70.0 {
        fg
    } else if luma(bg) > 128.0 {
        [20, 20, 20]
    } else {
        [240, 240, 240]
    }
}

// ------------------------------------------------------------------ colors

pub fn fg(colors: Colors, c: Rgb) -> String {
    match colors {
        Colors::True => format!("\x1b[38;2;{};{};{}m", c[0], c[1], c[2]),
        Colors::Ansi256 => format!("\x1b[38;5;{}m", ansi256(c)),
        Colors::Ansi16 => {
            let i = ansi16(c);
            format!("\x1b[{}m", if i < 8 { 30 + i } else { 82 + i })
        }
    }
}

pub fn bg(colors: Colors, c: Rgb) -> String {
    match colors {
        Colors::True => format!("\x1b[48;2;{};{};{}m", c[0], c[1], c[2]),
        Colors::Ansi256 => format!("\x1b[48;5;{}m", ansi256(c)),
        Colors::Ansi16 => {
            let i = ansi16(c);
            format!("\x1b[{}m", if i < 8 { 40 + i } else { 92 + i })
        }
    }
}

const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];

fn dist(a: Rgb, b: Rgb) -> u32 {
    a.iter().zip(b).map(|(x, y)| (*x as i32 - y as i32).pow(2) as u32).sum()
}

/// The nearest of xterm's 256 colors (the 6x6x6 cube or the 24 grays).
pub fn ansi256(c: Rgb) -> u8 {
    let level = |v: u8| LEVELS.iter().enumerate().min_by_key(|(_, l)| (**l as i32 - v as i32).abs()).unwrap().0;
    let (r, g, b) = (level(c[0]), level(c[1]), level(c[2]));
    let cube = [LEVELS[r], LEVELS[g], LEVELS[b]];
    let avg = (c[0] as u32 + c[1] as u32 + c[2] as u32) / 3;
    let gray_i = ((avg.saturating_sub(8)) / 10).min(23) as u8;
    let gray = 8 + 10 * gray_i;
    if dist(c, [gray; 3]) < dist(c, cube) { 232 + gray_i } else { (16 + 36 * r + 6 * g + b) as u8 }
}

const ANSI16: [Rgb; 16] = [
    [0, 0, 0],
    [170, 0, 0],
    [0, 170, 0],
    [170, 85, 0],
    [0, 0, 170],
    [170, 0, 170],
    [0, 170, 170],
    [170, 170, 170],
    [85, 85, 85],
    [255, 85, 85],
    [85, 255, 85],
    [255, 255, 85],
    [85, 85, 255],
    [255, 85, 255],
    [85, 255, 255],
    [255, 255, 255],
];

pub fn ansi16(c: Rgb) -> u8 {
    (0..16).min_by_key(|&i| dist(c, ANSI16[i])).unwrap() as u8
}

// ------------------------------------------------------------------ output

/// The escape codes that turn `old` into `new` (only the cells that changed) inside `area`.
pub fn diff(old: Option<&[Cell]>, new: &[Cell], area: Area, colors: Colors) -> String {
    let mut out = String::new();
    let (mut last_fg, mut last_bg, mut last_bold): (Option<Rgb>, Option<Rgb>, bool) = (None, None, false);
    let cols = area.cols as usize;
    // the cell the cursor is on after the last one written
    let mut cursor: Option<usize> = None;
    for (i, cell) in new.iter().enumerate() {
        if old.is_some_and(|o| o.get(i) == Some(cell)) {
            continue;
        }
        let (c, r) = (i % cols, i / cols);
        if cursor != Some(i) || c == 0 {
            let _ = write!(out, "\x1b[{};{}H", area.row as usize + r + 1, area.col as usize + c + 1);
        }
        if cell.bold != last_bold {
            out.push_str(if cell.bold { "\x1b[1m" } else { "\x1b[22m" });
            last_bold = cell.bold;
        }
        if cell.ch != ' ' && last_fg.is_none_or(|f| !same(colors, f, cell.fg)) {
            out.push_str(&fg(colors, cell.fg));
            last_fg = Some(cell.fg);
        }
        if last_bg.is_none_or(|b| !same(colors, b, cell.bg)) {
            out.push_str(&bg(colors, cell.bg));
            last_bg = Some(cell.bg);
        }
        out.push(cell.ch);
        cursor = Some(i + 1);
    }
    if !out.is_empty() {
        out.push_str("\x1b[0m");
    }
    out
}

fn same(colors: Colors, a: Rgb, b: Rgb) -> bool {
    match colors {
        Colors::True => a == b,
        Colors::Ansi256 => ansi256(a) == ansi256(b),
        Colors::Ansi16 => ansi16(a) == ansi16(b),
    }
}

/// The PNG drawn over `area` with the kitty graphics protocol; image 1 is replaced in each frame. `z=-1`: under
/// the text, so the help and the menus can be drawn over it.
pub fn kitty(png: &[u8], area: Area) -> String {
    let data = base64::engine::general_purpose::STANDARD.encode(png);
    let mut out = format!("\x1b[{};{}H", area.row + 1, area.col + 1);
    let chunks: Vec<&str> = data.as_bytes().chunks(4096).map(|c| std::str::from_utf8(c).unwrap()).collect();
    for (i, chunk) in chunks.iter().enumerate() {
        let more = if i + 1 < chunks.len() { 1 } else { 0 };
        if i == 0 {
            let _ = write!(
                out,
                "\x1b_Ga=T,f=100,i=1,p=1,q=2,z=-1,C=1,c={},r={},m={more};{chunk}\x1b\\",
                area.cols, area.rows
            );
        } else {
            let _ = write!(out, "\x1b_Gm={more};{chunk}\x1b\\");
        }
    }
    out
}

/// Removes the kitty image (on exit or when the screen shows something else).
pub fn kitty_clear() -> &'static str {
    "\x1b_Ga=d,d=I,i=1,q=2\x1b\\"
}

/// A sixel image of `img` at `area` (216-color cube, no dithering: fast and stable between frames).
pub fn sixel(img: &Image, area: Area) -> String {
    let cube = |v: u8| ((v as u32 * 5 + 127) / 255) as usize;
    let index: Vec<usize> = img.px.iter().map(|p| 36 * cube(p[0]) + 6 * cube(p[1]) + cube(p[2])).collect();
    let mut out = format!("\x1b[{};{}H\x1bP0;1;0q\"1;1;{};{}", area.row + 1, area.col + 1, img.width, img.height);
    let mut used = [false; 216];
    index.iter().for_each(|&i| used[i] = true);
    for (i, _) in used.iter().enumerate().filter(|(_, u)| **u) {
        let pct = |l: usize| l * 100 / 5;
        let _ = write!(out, "#{i};2;{};{};{}", pct(i / 36), pct(i / 6 % 6), pct(i % 6));
    }
    for band in (0..img.height).step_by(6) {
        let rows = (band..(band + 6).min(img.height)).collect::<Vec<_>>();
        let mut colors: Vec<usize> =
            rows.iter().flat_map(|&y| index[y * img.width..(y + 1) * img.width].iter().copied()).collect();
        colors.sort_unstable();
        colors.dedup();
        for (n, &color) in colors.iter().enumerate() {
            let _ = write!(out, "#{color}");
            let mut run: Option<(u8, usize)> = None;
            for x in 0..img.width {
                let bits = rows
                    .iter()
                    .enumerate()
                    .fold(0u8, |b, (k, &y)| if index[y * img.width + x] == color { b | (1 << k) } else { b });
                match run {
                    Some((b, n)) if b == bits => run = Some((b, n + 1)),
                    _ => {
                        if let Some((b, n)) = run {
                            push_sixel(&mut out, b, n);
                        }
                        run = Some((bits, 1));
                    }
                }
            }
            if let Some((b, n)) = run {
                push_sixel(&mut out, b, n);
            }
            out.push(if n + 1 < colors.len() { '$' } else { '-' });
        }
    }
    out.push_str("\x1b\\");
    out
}

fn push_sixel(out: &mut String, bits: u8, n: usize) {
    let ch = (63 + bits) as char;
    if n > 3 {
        let _ = write!(out, "!{n}{ch}");
    } else {
        (0..n).for_each(|_| out.push(ch));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: usize, h: usize, c: Rgb) -> Image {
        Image { width: w, height: h, px: vec![c; w * h] }
    }

    #[test]
    fn the_area_keeps_the_device_aspect() {
        let avail = Area { col: 1, row: 1, cols: 100, rows: 40 };
        // cells twice as tall as wide: a 1:2 device fills the height with as many columns as rows
        let a = fit((400, 800), avail, (8.0, 16.0));
        assert_eq!((a.rows, a.cols), (40, 40));
        assert_eq!(a.col, 1 + 30);
        // a wide device is limited by the width
        let b = fit((1920, 1080), Area { col: 0, row: 0, cols: 60, rows: 40 }, (8.0, 16.0));
        assert_eq!(b.cols, 60);
        assert!(b.rows < 40);
    }

    #[test]
    fn blocks_use_two_pixels_per_cell_and_texts_replace_them() {
        let mut img = solid(4, 8, [255, 255, 255]);
        for x in 0..4 {
            img.px[x] = [0, 0, 255]; // top row blue
        }
        let area = Area { col: 0, row: 0, cols: 4, rows: 4 };
        let c = cells(&img, area, (40, 40), &[]);
        assert_eq!(c[0].ch, '▀');
        assert_eq!(c[0].fg, [0, 0, 255]);
        assert_eq!(c[5].ch, ' ');
        let run = TextRun { text: "Hi".into(), x: 0.0, y: 20.0, w: 20.0, h: 10.0, color: Some([0, 0, 0]), bold: true };
        let c = cells(&img, area, (40, 40), &[run]);
        assert_eq!((c[8].ch, c[9].ch), ('H', 'i'));
        assert_eq!(c[8].bg, [255, 255, 255]);
        assert!(c[8].bold);
    }

    #[test]
    fn unreadable_text_colors_become_black_or_white() {
        assert_eq!(readable([250, 250, 250], [255, 255, 255]), [20, 20, 20]);
        assert_eq!(readable([10, 10, 10], [0, 0, 0]), [240, 240, 240]);
        assert_eq!(readable([0, 0, 0], [255, 255, 255]), [0, 0, 0]);
    }

    #[test]
    fn color_palettes() {
        assert_eq!(ansi256([255, 0, 0]), 196);
        assert_eq!(ansi256([128, 128, 128]), 244);
        assert_eq!(ansi16([250, 250, 250]), 15);
        assert_eq!(fg(Colors::Ansi16, [170, 0, 0]), "\x1b[31m");
        assert_eq!(bg(Colors::Ansi16, [255, 85, 85]), "\x1b[101m");
    }

    #[test]
    fn diff_writes_only_what_changed() {
        let area = Area { col: 2, row: 3, cols: 2, rows: 1 };
        let a = vec![Cell { ch: ' ', fg: [0; 3], bg: [0; 3], bold: false }; 2];
        let mut b = a.clone();
        assert_eq!(diff(Some(&a), &b, area, Colors::True), "");
        b[1].bg = [255, 0, 0];
        let out = diff(Some(&a), &b, area, Colors::True);
        assert!(out.starts_with("\x1b[4;4H"), "{out:?}");
        assert!(out.contains("48;2;255;0;0"));
    }

    #[test]
    fn sixel_and_kitty_envelopes() {
        let s = sixel(&solid(2, 7, [255, 0, 0]), Area { col: 0, row: 0, cols: 1, rows: 1 });
        assert!(s.contains("\x1bP0;1;0q\"1;1;2;7"));
        assert!(s.contains("#180;2;100;0;0"));
        assert!(s.ends_with("\x1b\\"));
        let k = kitty(&[1, 2, 3], Area { col: 4, row: 1, cols: 10, rows: 5 });
        assert!(k.starts_with("\x1b[2;5H\x1b_Ga=T,f=100,i=1"));
        assert!(k.contains("c=10,r=5,m=0;AQID"));
    }
}
