use std::collections::HashMap;

use gt_core::model::{FillKind, ObjectKind};
use tiny_skia::{
    FillRule, GradientStop, LinearGradient, Mask, Paint, PathBuilder, Pixmap, PixmapPaint, Point,
    RadialGradient, Rect, SpreadMode, Stroke, Transform,
};

use crate::font::{FontCatalog, draw_text};
use crate::load::crop_of;
use crate::model::{ImageFrame, Node, Raster, Scene, SizeMode};

pub fn rasterize(scene: &Scene, fonts: &mut FontCatalog) -> (Vec<Raster>, Vec<gt_core::Warning>) {
    let mut warnings = Vec::new();
    let mut rasters = Vec::new();
    for node in &scene.nodes {
        if matches!(node.paint, crate::model::Paint::Empty) {
            continue;
        }
        let mut built = raster_node(node, fonts, &mut warnings);
        rasters.append(&mut built);
    }
    apply_masks(scene, &mut rasters);
    (rasters, warnings)
}

fn raster_node(
    node: &Node,
    fonts: &mut FontCatalog,
    warnings: &mut Vec<gt_core::Warning>,
) -> Vec<Raster> {
    content_frames(node, fonts, warnings)
        .into_iter()
        .enumerate()
        .map(|(index, (pixmap, origin_x, origin_y, content_w, content_h))| {
            let id = if matches!(node.paint, crate::model::Paint::Image { ref frames, .. } if frames.len() > 1) {
                format!("{}#{index}", node.key)
            } else {
                node.key.clone()
            };
            pixmap_to_raster(
                id,
                pixmap,
                origin_x,
                origin_y,
                content_w as f32,
                content_h as f32,
            )
        })
        .collect()
}

fn content_frames(
    node: &Node,
    fonts: &mut FontCatalog,
    warnings: &mut Vec<gt_core::Warning>,
) -> Vec<(Pixmap, u32, u32, u32, u32)> {
    let width = node.rect.w.ceil().clamp(1.0, 8192.0) as u32;
    let height = node.rect.h.ceil().clamp(1.0, 8192.0) as u32;
    match &node.paint {
        crate::model::Paint::Text { color } => {
            let mut frames = Vec::new();
            let text_width = if node.ticker.is_some() {
                measure_ticker_width(node, fonts)
                    .ceil()
                    .clamp(width as f32, 16384.0) as u32
            } else {
                width
            };
            let mut pixmap = Pixmap::new(text_width.max(1), height.max(1))
                .unwrap_or_else(|| Pixmap::new(1, 1).expect("1x1 pixmap"));
            let fell_back = draw_text(
                fonts,
                &mut pixmap,
                &node.text,
                &node.font,
                &node.style,
                *color,
                text_width as f32,
                height as f32,
            );
            if fell_back {
                warnings.push(
                    gt_core::Warning::new(
                        "font.fallback",
                        format!(
                            "font '{}' is not installed; using a deterministic fallback",
                            node.font.family
                        ),
                    )
                    .with_object(&node.name),
                );
            }
            if node.ticker.is_some() && pixmap.width() > 1 {
                pixmap = duplicate_strip(pixmap);
            }
            let (pixmap, origin_x, origin_y) = finish_pixmap(node, pixmap);
            let content_w = if node.ticker.is_some() {
                (pixmap.width().saturating_sub(origin_x * 2) / 2).max(1)
            } else {
                pixmap.width().saturating_sub(origin_x * 2).max(1)
            };
            let content_h = pixmap.height().saturating_sub(origin_y * 2).max(1);
            frames.push((pixmap, origin_x, origin_y, content_w, content_h));
            frames
        }
        crate::model::Paint::Shape {
            kind,
            fill,
            stroke,
            radius,
            picture,
        } => {
            let (pixmap, origin_x, origin_y) = finish_pixmap(
                node,
                draw_shape(
                    width,
                    height,
                    *kind,
                    fill,
                    stroke,
                    *radius,
                    picture.as_ref(),
                ),
            );
            vec![(
                pixmap,
                origin_x,
                origin_y,
                pixmap_content_w(&node.rect, origin_x),
                pixmap_content_h(&node.rect, origin_y),
            )]
        }
        crate::model::Paint::Image { frames, size_mode } => {
            if frames.is_empty() {
                let pixmap =
                    Pixmap::new(width, height).unwrap_or_else(|| Pixmap::new(1, 1).expect("1x1"));
                vec![(pixmap, 0, 0, width, height)]
            } else {
                frames
                    .iter()
                    .map(|frame| {
                        let (pixmap, origin_x, origin_y) =
                            finish_pixmap(node, fit_image(width, height, frame, *size_mode));
                        let content_w = pixmap.width().saturating_sub(origin_x * 2).max(1);
                        let content_h = pixmap.height().saturating_sub(origin_y * 2).max(1);
                        (pixmap, origin_x, origin_y, content_w, content_h)
                    })
                    .collect()
            }
        }
        crate::model::Paint::Empty => Vec::new(),
    }
}

fn measure_ticker_width(node: &Node, fonts: &mut FontCatalog) -> f32 {
    crate::font::measure_text(fonts, &node.text, &node.font, &node.style, 100_000.0).width
        + node.rect.w
}

fn duplicate_strip(pixmap: Pixmap) -> Pixmap {
    let half = pixmap.width();
    let Some(mut doubled) = Pixmap::new(half.saturating_mul(2).max(1), pixmap.height()) else {
        return pixmap;
    };
    doubled.draw_pixmap(
        0,
        0,
        pixmap.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
    doubled.draw_pixmap(
        half as i32,
        0,
        pixmap.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
    doubled
}

fn pixmap_content_w(rect: &crate::model::Rect, origin: u32) -> u32 {
    rect.w.round().max(1.0) as u32 + origin.saturating_sub(origin)
}

fn pixmap_content_h(rect: &crate::model::Rect, _origin: u32) -> u32 {
    rect.h.round().max(1.0) as u32
}

fn finish_pixmap(node: &Node, mut pixmap: Pixmap) -> (Pixmap, u32, u32) {
    if let Some(crop) = crop_of(node) {
        apply_crop(&mut pixmap, crop);
    }
    let mut origin = 0_u32;
    if let Some(shadow) = &node.effects.shadow {
        let blur = (shadow.blur.unwrap_or(0.0) as u32).min(24);
        origin = blur.saturating_mul(2).saturating_add(2);
        pixmap = add_shadow(pixmap, blur, shadow_color(shadow));
    }
    if node.effects.reflection {
        pixmap = add_reflection(pixmap);
    }
    (pixmap, origin, origin)
}

fn shadow_color(shadow: &gt_core::model::ShadowEffect) -> [u8; 4] {
    shadow
        .color
        .map(|color| [color.r, color.g, color.b, color.a])
        .unwrap_or([0, 0, 0, 160])
}

fn draw_shape(
    width: u32,
    height: u32,
    kind: ObjectKind,
    fill: &gt_core::model::Fill,
    stroke: &gt_core::model::Stroke,
    radius: f32,
    picture: Option<&ImageFrame>,
) -> Pixmap {
    let mut pixmap = Pixmap::new(width.max(1), height.max(1))
        .unwrap_or_else(|| Pixmap::new(1, 1).expect("1x1 pixmap"));
    let path = shape_path(kind, pixmap.width() as f32, pixmap.height() as f32, radius);
    let Some(path) = path else {
        return pixmap;
    };
    let mut paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };
    match &fill.kind {
        FillKind::Solid { color } => {
            paint.set_color_rgba8(color.r, color.g, color.b, color.a);
            pixmap.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
        FillKind::LinearGradient { angle, wrap, stops } => {
            if let Some(shader) = linear_shader(
                pixmap.width() as f32,
                pixmap.height() as f32,
                *angle,
                wrap,
                stops,
            ) {
                paint.shader = shader;
                pixmap.fill_path(
                    &path,
                    &paint,
                    FillRule::Winding,
                    Transform::identity(),
                    None,
                );
            }
        }
        FillKind::RadialGradient { wrap, stops } => {
            if let Some(shader) =
                radial_shader(pixmap.width() as f32, pixmap.height() as f32, wrap, stops)
            {
                paint.shader = shader;
                pixmap.fill_path(
                    &path,
                    &paint,
                    FillRule::Winding,
                    Transform::identity(),
                    None,
                );
            }
        }
        FillKind::Picture { size_mode, .. } => {
            if let Some(frame) = picture {
                let image = fit_image(
                    pixmap.width(),
                    pixmap.height(),
                    frame,
                    SizeMode::parse(size_mode.as_deref()),
                );
                if let Some(mask) = Mask::new(pixmap.width(), pixmap.height()) {
                    let mut mask = mask;
                    mask.fill_path(&path, FillRule::Winding, true, Transform::identity());
                    pixmap.draw_pixmap(
                        0,
                        0,
                        image.as_ref(),
                        &PixmapPaint::default(),
                        Transform::identity(),
                        Some(&mask),
                    );
                }
            }
        }
        FillKind::Transparent | FillKind::Unsupported { .. } => {}
    }
    if let Some(thickness) = stroke.thickness
        && thickness > 0.0
        && let FillKind::Solid { color } = &stroke.fill.kind
    {
        let mut stroke_paint = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        stroke_paint.set_color_rgba8(color.r, color.g, color.b, color.a);
        let stroke = Stroke {
            width: thickness as f32,
            ..Stroke::default()
        };
        pixmap.stroke_path(&path, &stroke_paint, &stroke, Transform::identity(), None);
    }
    pixmap
}

fn shape_path(kind: ObjectKind, width: f32, height: f32, radius: f32) -> Option<tiny_skia::Path> {
    let rect = Rect::from_xywh(0.0, 0.0, width.max(1.0), height.max(1.0))?;
    match kind {
        ObjectKind::Ellipse => PathBuilder::from_oval(rect),
        ObjectKind::Triangle => {
            let mut builder = PathBuilder::new();
            builder.move_to(width * 0.5, 0.0);
            builder.line_to(width, height);
            builder.line_to(0.0, height);
            builder.close();
            builder.finish()
        }
        _ => {
            if radius > 0.0 {
                rounded_rect(width, height, radius)
            } else {
                Some(PathBuilder::from_rect(rect))
            }
        }
    }
}

fn rounded_rect(width: f32, height: f32, radius: f32) -> Option<tiny_skia::Path> {
    let radius = radius.min(width * 0.5).min(height * 0.5).max(0.0);
    if radius <= 0.0 {
        return Rect::from_xywh(0.0, 0.0, width, height).map(PathBuilder::from_rect);
    }
    let kappa = 0.552_284_8 * radius;
    let mut builder = PathBuilder::new();
    builder.move_to(radius, 0.0);
    builder.line_to(width - radius, 0.0);
    builder.cubic_to(
        width - radius + kappa,
        0.0,
        width,
        radius - kappa,
        width,
        radius,
    );
    builder.line_to(width, height - radius);
    builder.cubic_to(
        width,
        height - radius + kappa,
        width - radius + kappa,
        height,
        width - radius,
        height,
    );
    builder.line_to(radius, height);
    builder.cubic_to(
        radius - kappa,
        height,
        0.0,
        height - radius + kappa,
        0.0,
        height - radius,
    );
    builder.line_to(0.0, radius);
    builder.cubic_to(0.0, radius - kappa, radius - kappa, 0.0, radius, 0.0);
    builder.close();
    builder.finish()
}

fn linear_shader(
    width: f32,
    height: f32,
    angle: f64,
    wrap: &Option<String>,
    stops: &[gt_core::model::GradientStop],
) -> Option<tiny_skia::Shader<'static>> {
    let radians = angle.to_radians();
    let x1 = width * 0.5 - radians.cos() as f32 * width * 0.5;
    let y1 = height * 0.5 - radians.sin() as f32 * height * 0.5;
    let x2 = width * 0.5 + radians.cos() as f32 * width * 0.5;
    let y2 = height * 0.5 + radians.sin() as f32 * height * 0.5;
    LinearGradient::new(
        Point::from_xy(x1, y1),
        Point::from_xy(x2, y2),
        gradient_stops(stops)?,
        spread(wrap),
        Transform::identity(),
    )
}

fn radial_shader(
    width: f32,
    height: f32,
    wrap: &Option<String>,
    stops: &[gt_core::model::GradientStop],
) -> Option<tiny_skia::Shader<'static>> {
    RadialGradient::new(
        Point::from_xy(width * 0.5, height * 0.5),
        0.0,
        Point::from_xy(width * 0.5, height * 0.5),
        width.max(height) * 0.5,
        gradient_stops(stops)?,
        spread(wrap),
        Transform::identity(),
    )
}

fn gradient_stops(stops: &[gt_core::model::GradientStop]) -> Option<Vec<GradientStop>> {
    let mut ordered = stops.to_vec();
    ordered.sort_by(|left, right| left.offset.total_cmp(&right.offset));
    let values: Vec<GradientStop> = ordered
        .iter()
        .map(|stop| {
            GradientStop::new(
                stop.offset as f32,
                tiny_skia::Color::from_rgba8(
                    stop.color.r,
                    stop.color.g,
                    stop.color.b,
                    stop.color.a,
                ),
            )
        })
        .collect();
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

fn spread(wrap: &Option<String>) -> SpreadMode {
    match wrap.as_deref().unwrap_or("").to_ascii_lowercase().as_str() {
        "mirror" => SpreadMode::Reflect,
        "wrap" | "repeat" => SpreadMode::Repeat,
        _ => SpreadMode::Pad,
    }
}

fn fit_image(width: u32, height: u32, frame: &ImageFrame, mode: SizeMode) -> Pixmap {
    let mut pixmap = Pixmap::new(width.max(1), height.max(1))
        .unwrap_or_else(|| Pixmap::new(1, 1).expect("1x1 pixmap"));
    let Some(source) = premul_pixmap(frame) else {
        return pixmap;
    };
    let (dest_w, dest_h, x, y) = match mode {
        SizeMode::Stretch => (width as f32, height as f32, 0.0, 0.0),
        SizeMode::Centered => {
            let x = (width as f32 - source.width() as f32) * 0.5;
            let y = (height as f32 - source.height() as f32) * 0.5;
            (source.width() as f32, source.height() as f32, x, y)
        }
        SizeMode::Normal => (source.width() as f32, source.height() as f32, 0.0, 0.0),
        SizeMode::Contain => {
            let scale = (width as f32 / source.width() as f32)
                .min(height as f32 / source.height() as f32)
                .max(0.001);
            (
                source.width() as f32 * scale,
                source.height() as f32 * scale,
                0.0,
                0.0,
            )
        }
    };
    let transform = Transform::from_scale(
        dest_w / source.width() as f32,
        dest_h / source.height() as f32,
    );
    pixmap.draw_pixmap(
        x.round() as i32,
        y.round() as i32,
        source.as_ref(),
        &PixmapPaint::default(),
        transform,
        None,
    );
    pixmap
}

fn premul_pixmap(frame: &ImageFrame) -> Option<Pixmap> {
    let mut bytes = frame.rgba.clone();
    for pixel in bytes.as_chunks_mut::<4>().0 {
        let alpha = pixel[3] as u16;
        pixel[0] = (pixel[0] as u16 * alpha / 255) as u8;
        pixel[1] = (pixel[1] as u16 * alpha / 255) as u8;
        pixel[2] = (pixel[2] as u16 * alpha / 255) as u8;
    }
    Pixmap::from_vec(
        bytes,
        tiny_skia::IntSize::from_wh(frame.width.max(1), frame.height.max(1))?,
    )
}

fn apply_crop(pixmap: &mut Pixmap, crop: crate::model::Crop) {
    let width = pixmap.width();
    let height = pixmap.height();
    let data = pixmap.data_mut();
    for y in 0..height {
        for x in 0..width {
            let u = x as f32 / width as f32;
            let v = y as f32 / height as f32;
            let mut factor = 1.0_f32;
            factor *= edge_factor(u, crop.left, crop.feather);
            factor *= edge_factor(v, crop.top, crop.feather);
            factor *= edge_factor(1.0 - u, crop.right, crop.feather);
            factor *= edge_factor(1.0 - v, crop.bottom, crop.feather);
            let index = ((y * width + x) * 4) as usize;
            for channel in 0..4 {
                data[index + channel] = (data[index + channel] as f32 * factor).round() as u8;
            }
        }
    }
}

fn edge_factor(distance: f32, inset: f32, feather: f32) -> f32 {
    if distance >= inset {
        return 1.0;
    }
    if feather <= 0.0 {
        return 0.0;
    }
    ((distance - (inset - feather)) / feather).clamp(0.0, 1.0)
}

fn flip_pixmap(pixmap: Pixmap, flip_x: bool, flip_y: bool) -> Pixmap {
    let width = pixmap.width();
    let height = pixmap.height();
    let Some(mut out) = Pixmap::new(width, height) else {
        return pixmap;
    };
    let src = pixmap.data();
    let dst = out.data_mut();
    for y in 0..height {
        for x in 0..width {
            let sx = if flip_x { width - 1 - x } else { x };
            let sy = if flip_y { height - 1 - y } else { y };
            let from = ((sy * width + sx) * 4) as usize;
            let to = ((y * width + x) * 4) as usize;
            dst[to..to + 4].copy_from_slice(&src[from..from + 4]);
        }
    }
    out
}

fn add_shadow(pixmap: Pixmap, blur: u32, color: [u8; 4]) -> Pixmap {
    let pad = blur.saturating_mul(2).saturating_add(2);
    let width = pixmap.width() + pad * 2;
    let height = pixmap.height() + pad * 2;
    let Some(mut shadow) = Pixmap::new(width, height) else {
        return pixmap;
    };
    shadow.draw_pixmap(
        pad as i32,
        pad as i32,
        pixmap.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
    if blur > 0 {
        box_blur(&mut shadow, blur.min(24));
    }
    tint_by_alpha(&mut shadow, color);
    shadow.draw_pixmap(
        pad as i32,
        pad as i32,
        pixmap.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
    shadow
}

fn tint_by_alpha(pixmap: &mut Pixmap, color: [u8; 4]) {
    let data = pixmap.data_mut();
    for pixel in data.as_chunks_mut::<4>().0 {
        let alpha = pixel[3] as u16;
        let out_a = alpha * color[3] as u16 / 255;
        pixel[0] = (color[0] as u16 * out_a / 255) as u8;
        pixel[1] = (color[1] as u16 * out_a / 255) as u8;
        pixel[2] = (color[2] as u16 * out_a / 255) as u8;
        pixel[3] = out_a as u8;
    }
}

fn box_blur(pixmap: &mut Pixmap, radius: u32) {
    let radius = radius as i32;
    if radius <= 0 {
        return;
    }
    let width = pixmap.width() as i32;
    let height = pixmap.height() as i32;
    let mut src = pixmap.data().to_vec();
    let mut dst = vec![0_u8; src.len()];
    for y in 0..height {
        for x in 0..width {
            let mut accum = [0_u32; 4];
            let mut count = 0_u32;
            for offset in -radius..=radius {
                let sample = (x + offset).clamp(0, width - 1);
                let index = ((y * width + sample) * 4) as usize;
                for channel in 0..4 {
                    accum[channel] += src[index + channel] as u32;
                }
                count += 1;
            }
            let index = ((y * width + x) * 4) as usize;
            for channel in 0..4 {
                dst[index + channel] = (accum[channel] / count) as u8;
            }
        }
    }
    src.copy_from_slice(&dst);
    for y in 0..height {
        for x in 0..width {
            let mut accum = [0_u32; 4];
            let mut count = 0_u32;
            for offset in -radius..=radius {
                let sample = (y + offset).clamp(0, height - 1);
                let index = ((sample * width + x) * 4) as usize;
                for channel in 0..4 {
                    accum[channel] += src[index + channel] as u32;
                }
                count += 1;
            }
            let index = ((y * width + x) * 4) as usize;
            for channel in 0..4 {
                dst[index + channel] = (accum[channel] / count) as u8;
            }
        }
    }
    pixmap.data_mut().copy_from_slice(&dst);
}

fn add_reflection(pixmap: Pixmap) -> Pixmap {
    let gap = 2_u32;
    let reflected_h = pixmap.height() / 2;
    let height = pixmap.height() + gap + reflected_h;
    let Some(mut out) = Pixmap::new(pixmap.width(), height.max(1)) else {
        return pixmap;
    };
    out.draw_pixmap(
        0,
        0,
        pixmap.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
    let flipped = flip_pixmap(pixmap, false, true);
    let paint = PixmapPaint {
        opacity: 0.35,
        ..PixmapPaint::default()
    };
    out.draw_pixmap(
        0,
        (out.height() - reflected_h) as i32,
        flipped.as_ref(),
        &paint,
        Transform::identity(),
        None,
    );
    out
}

fn apply_masks(scene: &Scene, rasters: &mut [Raster]) {
    let by_name: HashMap<&str, &Node> = scene
        .nodes
        .iter()
        .map(|node| (node.name.as_str(), node))
        .collect();
    let by_id: HashMap<String, Raster> = rasters
        .iter()
        .cloned()
        .map(|raster| (raster.id.clone(), raster))
        .collect();
    for raster in rasters.iter_mut() {
        let Some(node) = scene
            .nodes
            .iter()
            .find(|node| node.key == raster.id || raster.id.starts_with(&format!("{}#", node.key)))
        else {
            continue;
        };
        let Some(mask_name) = node.effects.mask.as_deref() else {
            continue;
        };
        let Some(mask_node) = by_name.get(mask_name) else {
            continue;
        };
        let Some(mask) = by_id.get(&mask_node.key) else {
            continue;
        };
        multiply_mask(raster, node, mask, mask_node);
    }
}

fn multiply_mask(target: &mut Raster, node: &Node, mask: &Raster, mask_node: &Node) {
    let data = &mut target.rgba;
    for y in 0..target.height {
        for x in 0..target.width {
            let scene_x = node.rect.x + x as f32 - target.content_x as f32;
            let scene_y = node.rect.y + y as f32 - target.content_y as f32;
            let mask_x = (scene_x - mask_node.rect.x + mask.content_x as f32).round() as i32;
            let mask_y = (scene_y - mask_node.rect.y + mask.content_y as f32).round() as i32;
            let coverage = if mask_x >= 0
                && mask_y >= 0
                && (mask_x as u32) < mask.width
                && (mask_y as u32) < mask.height
            {
                mask.rgba[((mask_y as u32 * mask.width + mask_x as u32) * 4 + 3) as usize]
            } else {
                0
            } as u16;
            let index = ((y * target.width + x) * 4) as usize;
            for channel in 0..4 {
                data[index + channel] = (data[index + channel] as u16 * coverage / 255) as u8;
            }
        }
    }
    target.revision = revision(&target.rgba, target.width, target.height);
}

fn pixmap_to_raster(
    id: String,
    pixmap: Pixmap,
    content_x: u32,
    content_y: u32,
    content_w: f32,
    content_h: f32,
) -> Raster {
    let rgba = pixmap.data().to_vec();
    let width = pixmap.width();
    let height = pixmap.height();
    Raster {
        revision: revision(&rgba, width, height),
        id,
        width,
        height,
        rgba,
        content_x,
        content_y,
        content_w: content_w.round().max(1.0) as u32,
        content_h: content_h.round().max(1.0) as u32,
    }
}

fn revision(bytes: &[u8], width: u32, height: u32) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for value in [width, height] {
        for byte in value.to_le_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
