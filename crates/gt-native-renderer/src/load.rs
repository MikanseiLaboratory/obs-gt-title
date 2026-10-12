use std::path::{Path, PathBuf};

use image::AnimationDecoder;

use gt_core::model::{FillKind, GtDocument, Layer, LayerChild, ObjectKind};
use gt_core::package::{Package, PackageKind};
use gt_core::{ConvertOptions, parse, resolve};

use crate::error::Error;
use crate::model::{
    AnimKind, Animation, AutoSize, Axis, Blend, Crop, Direction, Easing, FontChoice, ImageFrame,
    Node, Paint, Rect, Scene, SizeMode, StoryKind, Storyboard, Ticker,
};

pub fn scene_from_package(package: &Package, mut document: GtDocument) -> Result<Scene, Error> {
    document.asset_names = package.asset_names();
    let mut warnings = resolve::collect_warnings(&document, &ConvertOptions::default());
    warnings.retain(|warning| {
        warning.code != "unsupported.storyboard.datachange"
            && warning.code != "unsupported.auto_size"
    });
    let mut nodes = Vec::new();
    for (index, layer) in document.layers.iter().enumerate() {
        walk_layer(
            package,
            layer,
            0.0,
            0.0,
            &format!("l{index}"),
            &[],
            &mut nodes,
        );
    }
    let storyboards = document
        .storyboards
        .iter()
        .enumerate()
        .map(|(index, storyboard)| {
            let kind = StoryKind::parse(storyboard.storyboard_type.as_deref());
            let label = storyboard
                .storyboard_type
                .clone()
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "TransitionIn".to_string());
            Storyboard {
                id: format!("{label}#{index}"),
                label,
                kind,
                animations: storyboard
                    .animations
                    .iter()
                    .map(|animation| Animation {
                        kind: AnimKind::parse(&animation.kind),
                        target: animation.object.clone().unwrap_or_default(),
                        duration: parse_seconds(animation.duration.as_deref(), 1.0),
                        delay: parse_seconds(animation.delay.as_deref(), 0.0),
                        easing: Easing::parse(animation.interpolation.as_deref()),
                        direction: Direction::parse(animation.direction.as_deref()),
                        reverse: animation.reversed,
                        center_axis: if animation
                            .center_axis
                            .as_deref()
                            .is_some_and(|axis| axis.eq_ignore_ascii_case("Y"))
                        {
                            Axis::Y
                        } else {
                            Axis::X
                        },
                    })
                    .collect(),
            }
        })
        .collect::<Vec<_>>();
    for node in &nodes {
        if node.effects.mask.is_some() {
            warnings.push(
                gt_core::Warning::new(
                    "approximate.mask.static",
                    "mask alpha is baked from the mask object's resting bounds",
                )
                .with_object(&node.name),
            );
        }
    }
    for node in &mut nodes {
        let sequence = storyboards
            .iter()
            .flat_map(|storyboard| &storyboard.animations)
            .find(|animation| {
                animation.kind == AnimKind::ImageSequence
                    && node.names.iter().any(|name| name == &animation.target)
            });
        if let Some(animation) = sequence {
            node.sequence_duration = animation.duration.max(0.05);
        } else if let Paint::Image { frames, .. } = &node.paint
            && frames.len() > 1
        {
            node.sequence_duration = (frames.len() as f32 / 8.0).max(0.05);
        }
    }
    let mut scene = Scene {
        width: document.width.max(1.0) as f32,
        height: document.height.max(1.0) as f32,
        nodes,
        storyboards,
        warnings,
    };
    crate::layout::layout(&mut scene, None);
    Ok(scene)
}

pub async fn load_path(path: impl AsRef<Path>) -> Result<Scene, Error> {
    let path = path.as_ref();
    let mut package = Package::open(path).await?;
    let document = parse::parse_document(&package.document_xml)?;
    package.load_external_images(&document).await?;
    scene_from_package(&package, document)
}

pub fn load_path_blocking(path: impl AsRef<Path>) -> Result<Scene, Error> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| Error::Message(error.to_string()))?;
    runtime.block_on(load_path(path))
}

pub fn load_xml(xml: &str) -> Result<Scene, Error> {
    let package = Package::from_xml_bytes(
        PathBuf::from("title.gtxml"),
        xml.as_bytes(),
        PackageKind::Gtxml,
    )?;
    let document = parse::parse_document(&package.document_xml)?;
    scene_from_package(&package, document)
}

fn walk_layer(
    package: &Package,
    layer: &Layer,
    origin_x: f32,
    origin_y: f32,
    path: &str,
    ancestors: &[String],
    out: &mut Vec<Node>,
) {
    let origin_x = origin_x + layer.location.x as f32;
    let origin_y = origin_y + layer.location.y as f32;
    let mut names = ancestors.to_vec();
    names.push(layer.name.clone());
    for (index, child) in layer.objects.iter().enumerate() {
        match child {
            LayerChild::Layer(nested) => {
                walk_layer(
                    package,
                    nested,
                    origin_x,
                    origin_y,
                    &format!("{path}_{index}"),
                    &names,
                    out,
                );
            }
            LayerChild::Object(object) => {
                if !object.kind.renders_html() {
                    continue;
                }
                let key = format!("{path}_{index}_{}", slug(&object.name));
                let mut node_names = names.clone();
                node_names.push(object.name.clone());
                let rect = Rect {
                    x: origin_x + object.location.x as f32,
                    y: origin_y + object.location.y as f32,
                    w: (object.dimensions.x as f32).max(1.0),
                    h: (object.dimensions.y as f32).max(1.0),
                };
                let text = object.text.clone().unwrap_or_default();
                let paint = match object.kind {
                    ObjectKind::TextBlock | ObjectKind::Text3D | ObjectKind::Ticker => {
                        Paint::Text {
                            color: solid_color(&object.fill),
                        }
                    }
                    ObjectKind::Rectangle | ObjectKind::Ellipse | ObjectKind::Triangle => {
                        let picture = match &object.fill.kind {
                            FillKind::Picture { source, .. } => {
                                decode_frames(package, source).into_iter().next()
                            }
                            _ => None,
                        };
                        Paint::Shape {
                            kind: object.kind,
                            fill: Box::new(object.fill.clone()),
                            stroke: Box::new(object.stroke.clone()),
                            radius: object.radius.unwrap_or(0.0) as f32,
                            picture,
                        }
                    }
                    ObjectKind::Image | ObjectKind::QrCode | ObjectKind::ImageSequence => {
                        let frames = object
                            .image_source
                            .as_deref()
                            .map(|source| decode_frames(package, source))
                            .unwrap_or_default();
                        if object.kind == ObjectKind::QrCode && frames.is_empty() {
                            Paint::Empty
                        } else {
                            Paint::Image {
                                frames,
                                size_mode: SizeMode::parse(object.size_mode.as_deref()),
                            }
                        }
                    }
                    ObjectKind::Unknown => Paint::Empty,
                };
                let (bounding_of, padding) = object
                    .bounding
                    .as_ref()
                    .map(|bounding| {
                        (
                            Some(bounding.object.clone()),
                            parse_padding(&bounding.padding),
                        )
                    })
                    .unwrap_or((None, (0.0, 0.0, 0.0, 0.0)));
                out.push(Node {
                    key,
                    name: object.name.clone(),
                    kind: object.kind,
                    names: node_names,
                    authored: rect,
                    rect,
                    rotation: object.rotate.unwrap_or(0.0) as f32,
                    opacity: normalize_opacity(object.opacity),
                    visible: object.visible,
                    paint,
                    effects: object.effects.clone(),
                    style: object.style.clone(),
                    font: FontChoice::from_style(&object.style),
                    text,
                    bounding_of,
                    padding,
                    auto_size: AutoSize::parse(object.style.auto_size.as_deref()),
                    ticker: if object.kind == ObjectKind::Ticker {
                        Some(Ticker {
                            speed: object.ticker_speed.unwrap_or(2.0).max(0.1) as f32,
                            direction: Direction::parse(object.ticker_direction.as_deref()),
                        })
                    } else {
                        None
                    },
                    sequence_duration: 1.0,
                });
            }
        }
    }
}

fn decode_frames(package: &Package, source: &str) -> Vec<ImageFrame> {
    let entries = package.sequence_entries(source);
    let sources = if entries.is_empty() {
        vec![source.to_string()]
    } else {
        entries
    };
    sources
        .iter()
        .filter_map(|entry| package.lookup_bytes(entry))
        .flat_map(decode_sequence)
        .collect()
}

fn decode_image(bytes: &[u8]) -> Option<ImageFrame> {
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    Some(ImageFrame {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    })
}

pub fn decode_sequence(bytes: &[u8]) -> Vec<ImageFrame> {
    let gif = bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a");
    if gif
        && let Ok(decoder) = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(bytes))
        && let Ok(frames) = decoder.into_frames().collect::<Result<Vec<_>, _>>()
        && !frames.is_empty()
    {
        return frames
            .into_iter()
            .map(|frame| {
                let buffer = frame.into_buffer();
                ImageFrame {
                    width: buffer.width(),
                    height: buffer.height(),
                    rgba: buffer.into_raw(),
                }
            })
            .collect();
    }
    decode_image(bytes).into_iter().collect()
}

fn solid_color(fill: &gt_core::model::Fill) -> [u8; 4] {
    match &fill.kind {
        FillKind::Solid { color } => [color.r, color.g, color.b, color.a],
        _ => [255, 255, 255, 255],
    }
}

fn normalize_opacity(value: Option<f64>) -> f32 {
    match value {
        None => 1.0,
        Some(value) if value > 1.0 => (value as f32 / 100.0).clamp(0.0, 1.0),
        Some(value) => (value as f32).clamp(0.0, 1.0),
    }
}

pub fn parse_padding(raw: &str) -> (f32, f32, f32, f32) {
    let parts: Vec<f32> = raw
        .split(',')
        .map(|part| part.trim().parse().unwrap_or(0.0))
        .collect();
    match parts.as_slice() {
        [one] => (*one, *one, *one, *one),
        [horizontal, vertical] => (*horizontal, *vertical, *horizontal, *vertical),
        [left, top, right, bottom] => (*left, *top, *right, *bottom),
        _ => (0.0, 0.0, 0.0, 0.0),
    }
}

pub fn parse_seconds(raw: Option<&str>, default: f32) -> f32 {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return default;
    };
    if let Ok(value) = raw.parse::<f32>() {
        return value.max(0.0);
    }
    let parts: Vec<&str> = raw.split(':').collect();
    if parts.len() == 3
        && let (Ok(hours), Ok(minutes), Ok(seconds)) = (
            parts[0].parse::<f32>(),
            parts[1].parse::<f32>(),
            parts[2].parse::<f32>(),
        )
    {
        return (hours * 3600.0 + minutes * 60.0 + seconds).max(0.0);
    }
    default
}

fn slug(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push('_');
    }
    out
}

pub fn blend_of(node: &Node) -> Blend {
    match node
        .effects
        .compositing
        .as_deref()
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("additive") => Blend::Additive,
        Some("replace") => Blend::Replace,
        _ => Blend::Normal,
    }
}

pub fn crop_of(node: &Node) -> Option<Crop> {
    let crop = node.effects.crop.as_ref()?;
    let range = crop.range.as_deref().unwrap_or("0,0,0,0");
    let (left, top, right, bottom) = parse_padding(range);
    let feather = crop
        .feather
        .as_deref()
        .and_then(|value| value.split(',').next())
        .and_then(|value| value.trim().parse::<f32>().ok())
        .unwrap_or(0.0)
        .max(0.0);
    Some(Crop {
        left,
        top,
        right,
        bottom,
        feather,
    })
}
