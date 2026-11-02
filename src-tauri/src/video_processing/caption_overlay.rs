//! Caption export data validated before native texture allocation.
use anyhow::{Result, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

const IMAGE_LIMIT: usize = 128 * 1024 * 1024;
const INSTRUCTION_LIMIT: usize = 1_000_000;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Part {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub opacity: f64,
    #[serde(rename = "blurRadius")]
    pub blur_radius: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Word {
    pub tile: usize,
    #[serde(flatten)]
    pub part: Part,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    pub start_frame: u64,
    pub end_frame: u64,
    pub background: Option<Part>,
    pub words: Vec<Word>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tile {
    pub png: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptionOverlay {
    pub width: u32,
    pub height: u32,
    pub frames: u64,
    pub tiles: Vec<Tile>,
    pub spans: Vec<Span>,
    #[serde(rename = "backgroundColor")]
    pub background_color: String,
    #[serde(rename = "cornerRadius")]
    pub corner_radius: f64,
}
pub(super) struct Raster {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}
pub(super) struct Prepared {
    pub tiles: Vec<Raster>,
    pub spans: Vec<Span>,
    pub background_color: [f32; 4],
    pub corner_radius: f64,
}
fn color(value: &str) -> Result<[f32; 4]> {
    let hex = value.strip_prefix('#').unwrap_or("");
    ensure!(
        (hex.len() == 6 || hex.len() == 8) && hex.bytes().all(|b| b.is_ascii_hexdigit()),
        "Invalid caption color"
    );
    let mut rgba = [1.0; 4];
    for (index, component) in rgba.iter_mut().enumerate().take(hex.len() / 2) {
        *component = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)? as f32 / 255.0;
    }
    Ok(rgba)
}
fn validate_part(part: &Part, width: u32, height: u32, background: bool) -> Result<()> {
    ensure!(
        [
            part.x,
            part.y,
            part.width,
            part.height,
            part.opacity,
            part.blur_radius
        ]
        .iter()
        .all(|n| n.is_finite())
            && part.x.abs() <= width as f64 * 4.0
            && part.y.abs() <= height as f64 * 4.0
            && part.width >= 0.0
            && part.height >= 0.0
            && (background || (part.width > 0.0 && part.height > 0.0))
            && part.width <= width as f64 * 4.0
            && part.height <= height as f64 * 4.0
            && (0.0..=1.0).contains(&part.opacity)
            && (0.0..=10.0).contains(&part.blur_radius),
        "Invalid caption placement"
    );
    Ok(())
}
impl Prepared {
    pub fn new(input: CaptionOverlay, width: u32, height: u32, frames: u64) -> Result<Self> {
        ensure!(
            width > 0
                && height > 0
                && frames > 0
                && input.width == width
                && input.height == height
                && input.frames == frames,
            "Caption overlay does not match export dimensions or duration"
        );
        ensure!(
            input.corner_radius.is_finite() && (0.0..=16384.0).contains(&input.corner_radius),
            "Invalid caption corner radius"
        );
        let background_color = color(&input.background_color)?;
        ensure!(
            input.spans.len() <= INSTRUCTION_LIMIT && input.tiles.len() <= INSTRUCTION_LIMIT,
            "Caption plan exceeds memory limit"
        );
        let mut previous = 0;
        let mut instructions = 0usize;
        for span in &input.spans {
            ensure!(
                span.start_frame >= previous
                    && span.start_frame < span.end_frame
                    && span.end_frame <= frames,
                "Invalid caption animation range"
            );
            previous = span.end_frame;
            instructions = instructions
                .checked_add(span.words.len())
                .and_then(|n| n.checked_add(usize::from(span.background.is_some())))
                .ok_or_else(|| anyhow::anyhow!("Caption instruction overflow"))?;
            ensure!(
                instructions <= INSTRUCTION_LIMIT,
                "Caption plan exceeds memory limit"
            );
            if let Some(part) = &span.background {
                validate_part(part, width, height, true)?;
            }
            for word in &span.words {
                ensure!(word.tile < input.tiles.len(), "Invalid caption tile index");
                validate_part(&word.part, width, height, false)?;
            }
        }
        let encoded = input
            .tiles
            .iter()
            .try_fold(0usize, |sum, tile| sum.checked_add(tile.png.len()))
            .ok_or_else(|| anyhow::anyhow!("Caption image overflow"))?;
        ensure!(
            encoded <= IMAGE_LIMIT,
            "Encoded captions exceed memory limit"
        );
        // Inspect every PNG header and aggregate decoded bytes before decoding any image.
        let mut allocated = 0usize;
        let mut images = Vec::with_capacity(input.tiles.len());
        for tile in input.tiles {
            let png = STANDARD.decode(tile.png)?;
            let reader = image::ImageReader::with_format(
                std::io::Cursor::new(&png),
                image::ImageFormat::Png,
            );
            let (w, h) = reader.into_dimensions()?;
            ensure!(
                w > 0 && h > 0 && w <= 16384 && h <= 16384,
                "Invalid caption raster dimensions"
            );
            let bytes = (w as usize)
                .checked_mul(h as usize)
                .and_then(|n| n.checked_mul(4))
                .ok_or_else(|| anyhow::anyhow!("Caption raster overflow"))?;
            allocated = allocated
                .checked_add(bytes)
                .ok_or_else(|| anyhow::anyhow!("Caption raster overflow"))?;
            ensure!(
                allocated <= IMAGE_LIMIT,
                "Caption images exceed memory limit"
            );
            images.push((w, h, png));
        }
        let tiles = images
            .into_iter()
            .map(|(width, height, png)| {
                Ok(Raster {
                    width,
                    height,
                    pixels: image::load_from_memory_with_format(&png, image::ImageFormat::Png)?
                        .into_rgba8()
                        .into_raw(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            tiles,
            spans: input.spans,
            background_color,
            corner_radius: input.corner_radius,
        })
    }
    pub fn at(&self, frame: u64) -> Option<&Span> {
        let index = self.spans.partition_point(|span| span.start_frame <= frame);
        index
            .checked_sub(1)
            .and_then(|i| self.spans.get(i))
            .filter(|span| frame < span.end_frame)
    }
}

#[cfg(test)]
#[path = "caption_overlay_tests.rs"]
mod tests;
