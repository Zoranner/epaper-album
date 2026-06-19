use std::path::PathBuf;

use fontdue::{
    layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle},
    Font, FontSettings,
};
use image::{DynamicImage, Rgba, RgbaImage};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::error::AppError;

use super::{image_processing::encode_rgb_bmp, paths::sprite_cache_path, state::RuntimeState};

const SPRITE_FONT_DIR: &str = "assets/fonts";
const SPRITE_FONT_CONFIG_PATH: &str = "assets/fonts.toml";

#[derive(Debug, Clone, Copy)]
pub(super) enum SpriteKind {
    Caption,
    Date,
    Status,
}

impl SpriteKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Caption => "caption",
            Self::Date => "date",
            Self::Status => "status",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub(super) struct SpriteStyle {
    font_size: f32,
    padding_x: u32,
    padding_y: u32,
    background: SpriteColor,
    color: SpriteColor,
    border_color: SpriteColor,
    border_width: u32,
}

#[derive(Debug)]
struct SpriteFontAsset {
    path: PathBuf,
}

#[derive(Debug, Deserialize)]
pub(super) struct SpriteFontConfig {
    files: Vec<String>,
    style: SpriteStyle,
}

#[derive(Debug)]
pub(super) struct LoadedSpriteFontConfig {
    pub raw: String,
    pub parsed: SpriteFontConfig,
}

struct SpriteCanvas<'a> {
    image: &'a mut RgbaImage,
    width: u32,
    height: u32,
    style: SpriteStyle,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum SpriteColor {
    Black,
    White,
    Red,
    Yellow,
    Blue,
    Green,
}

impl SpriteColor {
    const fn rgba(self) -> Rgba<u8> {
        match self {
            Self::Black => Rgba([0, 0, 0, 255]),
            Self::White => Rgba([255, 255, 255, 255]),
            Self::Red => Rgba([255, 0, 0, 255]),
            Self::Yellow => Rgba([255, 255, 0, 255]),
            Self::Blue => Rgba([0, 0, 255, 255]),
            Self::Green => Rgba([0, 255, 0, 255]),
        }
    }
}

pub(super) async fn ensure_sprite_cached(
    state: &RuntimeState,
    sha256: &str,
    text: String,
    font_config: LoadedSpriteFontConfig,
) -> Result<(), AppError> {
    let cache_path = sprite_cache_path(&state.app.data_dir, sha256);

    if cache_path.exists() {
        return Ok(());
    }

    let font_assets = load_sprite_font_assets(&font_config.parsed).await?;
    let mut font_bytes = Vec::with_capacity(font_assets.len());
    for asset in font_assets {
        font_bytes.push(
            tokio::fs::read(asset.path)
                .await
                .map_err(|error| AppError::Internal(error.into()))?,
        );
    }
    let style = font_config.parsed.style;
    let bmp = tokio::task::spawn_blocking(move || render_sprite_bmp(&text, font_bytes, style))
        .await
        .map_err(|error| AppError::Internal(error.into()))??;

    write_sprite_cache(&cache_path, &bmp).await?;
    Ok(())
}

pub(super) async fn load_sprite_font_config() -> Result<LoadedSpriteFontConfig, AppError> {
    let config = tokio::fs::read_to_string(SPRITE_FONT_CONFIG_PATH)
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    let parsed = toml::from_str(&config).map_err(|error| AppError::Internal(error.into()))?;
    Ok(LoadedSpriteFontConfig {
        raw: config,
        parsed,
    })
}

pub(super) fn validate_sprite_font_config(config: &SpriteFontConfig) -> Result<(), AppError> {
    if config
        .files
        .iter()
        .all(|file_name| file_name.trim().is_empty())
    {
        return Err(AppError::Internal(anyhow::anyhow!(
            "sprite font config has no font files"
        )));
    }
    if config.style.font_size <= 0.0 {
        return Err(AppError::Internal(anyhow::anyhow!(
            "sprite font size must be positive"
        )));
    }
    Ok(())
}

pub(super) fn sprite_sha256(kind: SpriteKind, text: &str, font_config: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(kind.as_str().as_bytes());
    hasher.update(text.as_bytes());
    hasher.update(font_config.as_bytes());
    hex::encode(hasher.finalize())
}

async fn load_sprite_font_assets(
    config: &SpriteFontConfig,
) -> Result<Vec<SpriteFontAsset>, AppError> {
    let mut assets = Vec::new();
    for file_name in &config.files {
        let file_name = file_name.trim();
        if file_name.is_empty() {
            continue;
        }
        let path = PathBuf::from(SPRITE_FONT_DIR).join(file_name);
        tokio::fs::metadata(&path)
            .await
            .map_err(|error| AppError::Internal(error.into()))?;
        assets.push(SpriteFontAsset { path });
    }
    if assets.is_empty() {
        return Err(AppError::Internal(anyhow::anyhow!(
            "sprite font config has no font files"
        )));
    }
    Ok(assets)
}

fn render_sprite_bmp(
    text: &str,
    font_bytes: Vec<Vec<u8>>,
    style: SpriteStyle,
) -> anyhow::Result<Vec<u8>> {
    let fonts = font_bytes
        .into_iter()
        .map(|bytes| {
            Font::from_bytes(bytes, FontSettings::default())
                .map_err(|error| anyhow::anyhow!("{error}"))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
    layout.reset(&LayoutSettings {
        x: style.padding_x as f32,
        y: style.padding_y as f32,
        ..LayoutSettings::default()
    });
    for character in text.chars() {
        let font_index = fallback_font_index(&fonts, character);
        layout.append(
            &fonts,
            &TextStyle::new(&character.to_string(), style.font_size, font_index),
        );
    }

    let glyphs = layout.glyphs();
    let text_width = glyphs
        .iter()
        .map(|glyph| (glyph.x + glyph.width as f32).ceil() as i32)
        .max()
        .unwrap_or(style.padding_x as i32);
    let text_height = glyphs
        .iter()
        .map(|glyph| (glyph.y + glyph.height as f32).ceil() as i32)
        .max()
        .unwrap_or(style.padding_y as i32);
    let width = (text_width.max(style.padding_x as i32) as u32 + style.padding_x).max(1);
    let height = (text_height.max(style.padding_y as i32) as u32 + style.padding_y).max(1);
    let mut image = RgbaImage::from_pixel(width, height, style.background.rgba());

    for glyph in glyphs {
        let (metrics, bitmap) = fonts[glyph.font_index].rasterize_config(glyph.key);
        let left = glyph.x.round() as i32;
        let top = glyph.y.round() as i32;

        let mut canvas = SpriteCanvas {
            image: &mut image,
            width,
            height,
            style,
        };
        draw_glyph_stroke(&mut canvas, left, top, &metrics, &bitmap);
        draw_glyph_fill(&mut canvas, left, top, &metrics, &bitmap);
    }

    encode_rgb_bmp(&DynamicImage::ImageRgba8(image).to_rgb8())
}

fn draw_glyph_stroke(
    canvas: &mut SpriteCanvas<'_>,
    left: i32,
    top: i32,
    metrics: &fontdue::Metrics,
    bitmap: &[u8],
) {
    let stroke_width = canvas.style.border_width.min(4) as i32;
    if stroke_width == 0 {
        return;
    }

    for y in 0..metrics.height {
        for x in 0..metrics.width {
            let coverage = bitmap[y * metrics.width + x];
            if coverage < 32 {
                continue;
            }

            for offset_y in -stroke_width..=stroke_width {
                for offset_x in -stroke_width..=stroke_width {
                    let target_x = left + x as i32 + offset_x;
                    let target_y = top + y as i32 + offset_y;
                    put_sprite_pixel(
                        canvas.image,
                        canvas.width,
                        canvas.height,
                        target_x,
                        target_y,
                        canvas.style.border_color.rgba(),
                    );
                }
            }
        }
    }
}

fn draw_glyph_fill(
    canvas: &mut SpriteCanvas<'_>,
    left: i32,
    top: i32,
    metrics: &fontdue::Metrics,
    bitmap: &[u8],
) {
    for y in 0..metrics.height {
        for x in 0..metrics.width {
            let coverage = bitmap[y * metrics.width + x];
            if coverage < 96 {
                continue;
            }

            let target_x = left + x as i32;
            let target_y = top + y as i32;
            put_sprite_pixel(
                canvas.image,
                canvas.width,
                canvas.height,
                target_x,
                target_y,
                canvas.style.color.rgba(),
            );
        }
    }
}

fn put_sprite_pixel(
    image: &mut RgbaImage,
    width: u32,
    height: u32,
    target_x: i32,
    target_y: i32,
    color: Rgba<u8>,
) {
    if target_x < 0 || target_y < 0 || target_x >= width as i32 || target_y >= height as i32 {
        return;
    }

    image.put_pixel(target_x as u32, target_y as u32, color);
}

fn fallback_font_index(fonts: &[Font], character: char) -> usize {
    fonts
        .iter()
        .position(|font| font.has_glyph(character))
        .unwrap_or(0)
}

async fn write_sprite_cache(path: &std::path::Path, bytes: &[u8]) -> Result<(), AppError> {
    let directory = path
        .parent()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("invalid sprite cache path")))?;
    tokio::fs::create_dir_all(directory)
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    let temp_path = path.with_extension("tmp");
    tokio::fs::write(&temp_path, bytes)
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    if path.exists() {
        tokio::fs::remove_file(path)
            .await
            .map_err(|error| AppError::Internal(error.into()))?;
    }
    tokio::fs::rename(temp_path, path)
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprite_style_accepts_panel_color_names_only() {
        let config = toml::from_str::<SpriteFontConfig>(
            r#"
files = ["TerminessTTF NF.ttf"]

[style]
font_size = 32.0
padding_x = 12
padding_y = 8
background = "green"
color = "white"
border_color = "black"
border_width = 1
"#,
        )
        .expect("parse panel color style");

        assert_eq!(config.style.background.rgba(), Rgba([0, 255, 0, 255]));
        assert_eq!(config.style.color.rgba(), Rgba([255, 255, 255, 255]));
        assert_eq!(config.style.border_color.rgba(), Rgba([0, 0, 0, 255]));

        let invalid = toml::from_str::<SpriteFontConfig>(
            r##"
files = ["TerminessTTF NF.ttf"]

[style]
font_size = 32.0
padding_x = 12
padding_y = 8
background = "#155e75"
color = "white"
border_color = "black"
border_width = 1
"##,
        );

        assert!(invalid.is_err());
    }
}
