use cosmic_text::{
    Attrs, Buffer, Family, FontSystem, Hinting, Metrics, Shaping, Stretch, Style, SwashCache,
    Weight,
};

use crate::model::FontChoice;

pub struct FontCatalog {
    pub system: FontSystem,
    pub cache: SwashCache,
}

pub struct ResolvedFont {
    pub family: String,
    pub fell_back: bool,
}

impl FontCatalog {
    pub fn system() -> Self {
        Self {
            system: FontSystem::new(),
            cache: SwashCache::new(),
        }
    }

    pub fn from_font_bytes(bytes: &[u8]) -> Self {
        let mut database = cosmic_text::fontdb::Database::new();
        database.load_font_data(bytes.to_vec());
        Self {
            system: FontSystem::new_with_locale_and_db("en-US".to_string(), database),
            cache: SwashCache::new(),
        }
    }

    pub fn resolve(&self, choice: &FontChoice) -> ResolvedFont {
        let requested = choice.family.trim();
        if !requested.is_empty()
            && !requested.eq_ignore_ascii_case("sans-serif")
            && family_available(&self.system, requested)
        {
            return ResolvedFont {
                family: requested.to_string(),
                fell_back: false,
            };
        }
        if requested.eq_ignore_ascii_case("sans-serif") || requested.is_empty() {
            return ResolvedFont {
                family: fallback_family(&self.system).unwrap_or_else(|| "sans-serif".to_string()),
                fell_back: false,
            };
        }
        ResolvedFont {
            family: fallback_family(&self.system).unwrap_or_else(|| requested.to_string()),
            fell_back: true,
        }
    }
}

fn family_available(system: &FontSystem, name: &str) -> bool {
    system
        .db()
        .query(&cosmic_text::fontdb::Query {
            families: &[Family::Name(name)],
            weight: Weight::NORMAL,
            stretch: Stretch::Normal,
            style: Style::Normal,
        })
        .is_some()
}

fn fallback_family(system: &FontSystem) -> Option<String> {
    let mut names = Vec::new();
    for face in system.db().faces() {
        for (name, _) in &face.families {
            names.push(name.clone());
        }
    }
    names.sort();
    names.dedup();
    names.into_iter().next()
}

pub struct TextMeasure {
    pub width: f32,
    pub height: f32,
}

pub fn measure_text(
    catalog: &mut FontCatalog,
    text: &str,
    choice: &FontChoice,
    style: &gt_core::model::TextStyle,
    max_width: f32,
) -> TextMeasure {
    let resolved = catalog.resolve(choice);
    let buffer = prepare_buffer(catalog, text, choice, style, &resolved.family, max_width);
    let mut width = 0.0_f32;
    let mut height = 0.0_f32;
    for run in buffer.layout_runs() {
        width = width.max(run.line_w);
        height = height.max(run.line_top + run.line_height);
    }
    TextMeasure { width, height }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_text(
    catalog: &mut FontCatalog,
    pixmap: &mut tiny_skia::Pixmap,
    text: &str,
    choice: &FontChoice,
    style: &gt_core::model::TextStyle,
    color: [u8; 4],
    box_width: f32,
    box_height: f32,
) -> bool {
    let resolved = catalog.resolve(choice);
    let mut buffer = prepare_buffer(catalog, text, choice, style, &resolved.family, box_width);
    let mut content_height = 0.0_f32;
    for run in buffer.layout_runs() {
        content_height = content_height.max(run.line_top + run.line_height);
    }
    let vertical = style
        .vertical_align
        .as_deref()
        .unwrap_or("Top")
        .to_ascii_lowercase();
    let y_offset = if vertical.contains("center") || vertical.contains("middle") {
        ((box_height - content_height) * 0.5).max(0.0)
    } else if vertical.contains("bottom") {
        (box_height - content_height).max(0.0)
    } else {
        0.0
    };
    let y_offset = y_offset.round() as i32;
    buffer.draw(
        &mut catalog.system,
        &mut catalog.cache,
        cosmic_text::Color::rgba(color[0], color[1], color[2], color[3]),
        |x, y, w, h, color| {
            for row in 0..h as i32 {
                for column in 0..w as i32 {
                    blit_glyph(pixmap, x + column, y + y_offset + row, color);
                }
            }
        },
    );
    resolved.fell_back
}

fn prepare_buffer(
    catalog: &mut FontCatalog,
    text: &str,
    choice: &FontChoice,
    style: &gt_core::model::TextStyle,
    family: &str,
    max_width: f32,
) -> Buffer {
    let size = choice.size.max(1.0);
    let extra = style.line_spacing.unwrap_or(size as f64 * 0.2) as f32;
    let metrics = Metrics::new(size, (size + extra).max(1.0));
    let mut buffer = Buffer::new(&mut catalog.system, metrics);
    buffer.set_hinting(Hinting::Disabled);
    let wrap = style
        .word_wrapping
        .as_deref()
        .is_some_and(|value| value.to_ascii_lowercase().contains("wrap"));
    buffer.set_wrap(if wrap {
        cosmic_text::Wrap::WordOrGlyph
    } else {
        cosmic_text::Wrap::None
    });
    let width = if max_width.is_finite() && max_width > 1.0 && wrap {
        Some(max_width)
    } else if choice_needs_bound(style) {
        Some(max_width.max(1.0))
    } else {
        None
    };
    buffer.set_size(width, None);
    let mut attrs = Attrs::new()
        .family(Family::Name(family))
        .weight(font_weight(style, choice.bold))
        .stretch(font_stretch(style))
        .style(if choice.italic {
            Style::Italic
        } else {
            Style::Normal
        });
    if choice.underline {
        attrs = attrs.underline(cosmic_text::UnderlineStyle::Single);
    }
    if choice.strikeout {
        attrs = attrs.strikethrough();
    }
    let display = if style.auto_upper_case {
        text.to_uppercase()
    } else {
        text.to_string()
    };
    let display = if style.rtl {
        format!("\u{200F}{display}")
    } else {
        display
    };
    let align = match style
        .text_align
        .as_deref()
        .unwrap_or("Left")
        .to_ascii_lowercase()
        .as_str()
    {
        "center" => Some(cosmic_text::Align::Center),
        "right" | "end" => Some(cosmic_text::Align::Right),
        _ => Some(cosmic_text::Align::Left),
    };
    buffer.set_text(&display, &attrs, Shaping::Advanced, align);
    buffer.shape_until_scroll(&mut catalog.system, false);
    buffer
}

fn choice_needs_bound(style: &gt_core::model::TextStyle) -> bool {
    style
        .text_align
        .as_deref()
        .is_some_and(|value| !value.eq_ignore_ascii_case("left"))
}

fn font_weight(style: &gt_core::model::TextStyle, bold: bool) -> Weight {
    if bold {
        return Weight::BOLD;
    }
    match style
        .font_weight
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "thin" => Weight::THIN,
        "light" => Weight::LIGHT,
        "medium" => Weight::MEDIUM,
        "semibold" | "semi-bold" | "demibold" => Weight::SEMIBOLD,
        "bold" => Weight::BOLD,
        "black" | "heavy" => Weight::BLACK,
        other => other.parse::<u16>().map(Weight).unwrap_or(Weight::NORMAL),
    }
}

fn font_stretch(style: &gt_core::model::TextStyle) -> Stretch {
    match style
        .font_stretch
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "ultra-condensed" | "ultracondensed" => Stretch::UltraCondensed,
        "extra-condensed" | "extracondensed" => Stretch::ExtraCondensed,
        "condensed" => Stretch::Condensed,
        "semi-condensed" | "semicondensed" => Stretch::SemiCondensed,
        "semi-expanded" | "semiexpanded" => Stretch::SemiExpanded,
        "expanded" => Stretch::Expanded,
        "extra-expanded" | "extraexpanded" => Stretch::ExtraExpanded,
        "ultra-expanded" | "ultraexpanded" => Stretch::UltraExpanded,
        _ => Stretch::Normal,
    }
}

fn blit_glyph(pixmap: &mut tiny_skia::Pixmap, x: i32, y: i32, color: cosmic_text::Color) {
    let (red, green, blue, alpha) = color.as_rgba_tuple();
    if alpha == 0 {
        return;
    }
    let width = pixmap.width() as i32;
    let height = pixmap.height() as i32;
    if x < 0 || y < 0 || x >= width || y >= height {
        return;
    }
    let index = ((y as u32 * pixmap.width() + x as u32) * 4) as usize;
    let data = pixmap.data_mut();
    let src_a = alpha as u16;
    let src_r = red as u16 * src_a / 255;
    let src_g = green as u16 * src_a / 255;
    let src_b = blue as u16 * src_a / 255;
    let dst_a = data[index + 3] as u16;
    let inv = 255 - src_a;
    data[index] = (src_r + data[index] as u16 * inv / 255) as u8;
    data[index + 1] = (src_g + data[index + 1] as u16 * inv / 255) as u8;
    data[index + 2] = (src_b + data[index + 2] as u16 * inv / 255) as u8;
    data[index + 3] = (src_a + dst_a * inv / 255) as u8;
}
