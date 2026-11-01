//! Reads image dimensions from file headers (PNG, GIF, JPEG, WebP, SVG) so
//! `<img>` tags can carry `width`/`height`. With those, browsers reserve the
//! space before the image loads and the page does not jump (no layout shift).

use std::fs;
use std::path::Path;

/// Returns `(width, height)` of the image at `path`, if it can be determined.
pub fn dimensions(path: &Path) -> Option<(u32, u32)> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if ext == "svg" {
        return svg_dimensions(&fs::read_to_string(path).ok()?);
    }
    raster_dimensions(&fs::read(path).ok()?)
}

fn raster_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    let be16 = |at: usize| Some(u16::from_be_bytes([*data.get(at)?, *data.get(at + 1)?]) as u32);
    let le16 = |at: usize| Some(u16::from_le_bytes([*data.get(at)?, *data.get(at + 1)?]) as u32);
    let be32 = |at: usize| {
        let bytes = data.get(at..at + 4)?;
        Some(u32::from_be_bytes(bytes.try_into().ok()?))
    };
    let le24 = |at: usize| {
        let b = data.get(at..at + 3)?;
        Some(b[0] as u32 | (b[1] as u32) << 8 | (b[2] as u32) << 16)
    };

    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some((be32(16)?, be32(20)?));
    }
    if data.starts_with(b"GIF8") {
        return Some((le16(6)?, le16(8)?));
    }
    if data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WEBP") {
        return match data.get(12..16)? {
            b"VP8X" => Some((le24(24)? + 1, le24(27)? + 1)),
            b"VP8 " => Some((le16(26)? & 0x3fff, le16(28)? & 0x3fff)),
            b"VP8L" => {
                let bits = u32::from_le_bytes(data.get(21..25)?.try_into().ok()?);
                Some(((bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1))
            }
            _ => None,
        };
    }
    if data.starts_with(&[0xff, 0xd8]) {
        // Walk the JPEG segments until a start-of-frame marker.
        let mut at = 2;
        while at + 9 < data.len() {
            if data[at] != 0xff {
                at += 1;
                continue;
            }
            let marker = data[at + 1];
            let is_sof = (0xc0..=0xcf).contains(&marker) && !matches!(marker, 0xc4 | 0xc8 | 0xcc);
            if is_sof {
                return Some((be16(at + 7)?, be16(at + 5)?));
            }
            at += 2 + be16(at + 2)? as usize;
        }
    }
    None
}

/// Reads `width`/`height` from the root `<svg>` element, falling back to `viewBox`.
fn svg_dimensions(svg: &str) -> Option<(u32, u32)> {
    let start = svg.find("<svg")?;
    let tag = &svg[start..start + svg[start..].find('>')?];
    let attr = |name: &str| {
        let needle = format!(" {name}=\"");
        let at = tag.find(&needle)? + needle.len();
        Some(&tag[at..at + tag[at..].find('"')?])
    };
    let number = |s: &str| {
        let digits: String = s
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        digits.parse::<f64>().ok().map(|n| n.round() as u32)
    };

    let width = attr("width").filter(|w| !w.ends_with('%')).and_then(number);
    let height = attr("height")
        .filter(|h| !h.ends_with('%'))
        .and_then(number);
    if let (Some(w), Some(h)) = (width, height) {
        return Some((w, h));
    }
    let view_box: Vec<f64> = attr("viewBox")?
        .split([' ', ','])
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();
    match view_box[..] {
        [_, _, w, h] if w > 0.0 && h > 0.0 => Some((w.round() as u32, h.round() as u32)),
        _ => None,
    }
}

/// Adds `width`/`height` to every `<img src="…">` in `html` whose file can be
/// found relative to `page_dir` (the directory of the page inside `out_dir`).
pub fn add_dimensions(html: &str, out_dir: &Path, page_dir: &Path) -> String {
    let mut out = String::with_capacity(html.len() + 64);
    let mut rest = html;
    while let Some(start) = rest.find("<img ") {
        let Some(end) = rest[start..].find('>').map(|e| start + e) else {
            break;
        };
        let tag = &rest[start..end];
        out.push_str(&rest[..start]);
        out.push_str(tag);
        if !tag.contains(" width=") {
            if let Some((w, h)) =
                src_of(tag).and_then(|src| local_dimensions(src, out_dir, page_dir))
            {
                out.push_str(&format!(" width=\"{w}\" height=\"{h}\""));
            }
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

fn src_of(tag: &str) -> Option<&str> {
    let at = tag.find(" src=\"")? + 6;
    Some(&tag[at..at + tag[at..].find('"')?])
}

fn local_dimensions(src: &str, out_dir: &Path, page_dir: &Path) -> Option<(u32, u32)> {
    if src.contains(':') || src.starts_with('/') {
        return None;
    }
    let path = page_dir.join(src.split(['?', '#']).next()?);
    // Stay inside the output directory.
    let path = fs::canonicalize(path).ok()?;
    path.starts_with(fs::canonicalize(out_dir).ok()?)
        .then(|| dimensions(&path))
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_png_and_gif_headers() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend_from_slice(&640u32.to_be_bytes());
        png.extend_from_slice(&480u32.to_be_bytes());
        assert_eq!(raster_dimensions(&png), Some((640, 480)));

        let gif = b"GIF89a\x20\x03\x58\x02";
        assert_eq!(raster_dimensions(gif), Some((800, 600)));
    }

    #[test]
    fn reads_jpeg_frame_header() {
        // SOI, APP0 (length 4), SOF0 with height 300 and width 400.
        let jpeg = [
            0xff, 0xd8, 0xff, 0xe0, 0x00, 0x04, 0x00, 0x00, 0xff, 0xc0, 0x00, 0x11, 0x08, 0x01,
            0x2c, 0x01, 0x90, 0x03,
        ];
        assert_eq!(raster_dimensions(&jpeg), Some((400, 300)));
    }

    #[test]
    fn reads_svg_size() {
        assert_eq!(
            svg_dimensions("<svg width=\"20px\" height=\"10\">"),
            Some((20, 10))
        );
        assert_eq!(
            svg_dimensions("<svg xmlns=\"x\" viewBox=\"0 0 640 160\">"),
            Some((640, 160))
        );
        assert_eq!(svg_dimensions("<svg width=\"100%\">"), None);
    }
}
