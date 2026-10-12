//! Intermediate scene shared by the rasterizer and the OBS source.

use gt_core::model::{Fill, ObjectEffects, ObjectKind, Stroke, TextStyle};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self {
            x: x as f32,
            y: y as f32,
            w: (w as f32).max(0.0),
            h: (h as f32).max(0.0),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FontChoice {
    pub family: String,
    pub style: String,
    pub size: f32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
}

impl FontChoice {
    pub fn from_style(style: &TextStyle) -> Self {
        let italic = style.italic
            || style
                .font_style
                .as_deref()
                .is_some_and(|value| value.to_ascii_lowercase().contains("italic"));
        let bold = style
            .font_weight
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case("bold"));
        Self {
            family: style
                .font_family
                .clone()
                .unwrap_or_else(|| "sans-serif".to_string()),
            style: style.font_style.clone().unwrap_or_else(|| {
                if bold && italic {
                    "Bold Italic".to_string()
                } else if bold {
                    "Bold".to_string()
                } else if italic {
                    "Italic".to_string()
                } else {
                    "Regular".to_string()
                }
            }),
            size: style.font_size.unwrap_or(32.0).max(1.0) as f32,
            bold,
            italic,
            underline: style.underline,
            strikeout: style.strikethrough,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoryKind {
    TransitionIn,
    TransitionOut,
    Continuous,
    DataChangeIn,
    DataChangeOut,
    Other,
}

impl StoryKind {
    pub fn parse(raw: Option<&str>) -> Self {
        match raw.unwrap_or("").trim().to_ascii_lowercase().as_str() {
            "" | "transitionin" => Self::TransitionIn,
            "transitionout" => Self::TransitionOut,
            "continuous" => Self::Continuous,
            "datachangein" => Self::DataChangeIn,
            "datachangeout" => Self::DataChangeOut,
            _ => Self::Other,
        }
    }

    pub fn exits(self) -> bool {
        matches!(self, Self::TransitionOut | Self::DataChangeOut)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimKind {
    Reveal,
    Fade,
    Fly,
    ZoomFade,
    Scale,
    Rotate,
    Spin,
    Wipe,
    ImageSequence,
    None,
    Other,
}

impl AnimKind {
    pub fn parse(raw: &str) -> Self {
        match raw {
            "Reveal" => Self::Reveal,
            "Fade" => Self::Fade,
            "Fly" | "Move" => Self::Fly,
            "ZoomFade" | "Zoom" => Self::ZoomFade,
            "Scale" => Self::Scale,
            "Rotate" | "Flip" => Self::Rotate,
            "Spin" => Self::Spin,
            "Wipe" => Self::Wipe,
            "ImageSequence" => Self::ImageSequence,
            "None" => Self::None,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Top,
    Bottom,
    Center,
}

impl Direction {
    pub fn parse(raw: Option<&str>) -> Self {
        match raw.unwrap_or("").trim().to_ascii_lowercase().as_str() {
            "right" => Self::Right,
            "top" => Self::Top,
            "bottom" => Self::Bottom,
            "center" => Self::Center,
            _ => Self::Left,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Easing {
    Linear,
    CubicIn,
    CubicOut,
    CubicInOut,
    BounceIn,
    BounceOut,
}

impl Easing {
    pub fn parse(raw: Option<&str>) -> Self {
        match raw.unwrap_or("").trim().to_ascii_lowercase().as_str() {
            "cubiceasingin" => Self::CubicIn,
            "cubiceasingout" => Self::CubicOut,
            "cubiceasinginout" => Self::CubicInOut,
            "bouncein" => Self::BounceIn,
            "bounceout" => Self::BounceOut,
            _ => Self::Linear,
        }
    }

    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::CubicIn => t * t * t,
            Self::CubicOut => {
                let u = 1.0 - t;
                1.0 - u * u * u
            }
            Self::CubicInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    let u = -2.0 * t + 2.0;
                    1.0 - u * u * u / 2.0
                }
            }
            Self::BounceOut => bounce_out(t),
            Self::BounceIn => 1.0 - bounce_out(1.0 - t),
        }
    }
}

fn bounce_out(t: f32) -> f32 {
    let n1 = 7.5625;
    let d1 = 2.75;
    if t < 1.0 / d1 {
        n1 * t * t
    } else if t < 2.0 / d1 {
        let t = t - 1.5 / d1;
        n1 * t * t + 0.75
    } else if t < 2.5 / d1 {
        let t = t - 2.25 / d1;
        n1 * t * t + 0.9375
    } else {
        let t = t - 2.625 / d1;
        n1 * t * t + 0.984375
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Animation {
    pub kind: AnimKind,
    pub target: String,
    pub duration: f32,
    pub delay: f32,
    pub easing: Easing,
    pub direction: Direction,
    pub reverse: bool,
    pub center_axis: Axis,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Storyboard {
    pub id: String,
    pub label: String,
    pub kind: StoryKind,
    pub animations: Vec<Animation>,
}

impl Storyboard {
    pub fn duration(&self) -> f32 {
        self.animations
            .iter()
            .map(|animation| animation.delay + animation.duration)
            .fold(0.0, f32::max)
            .max(0.001)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeMode {
    Contain,
    Normal,
    Stretch,
    Centered,
}

impl SizeMode {
    pub fn parse(raw: Option<&str>) -> Self {
        match raw.unwrap_or("").trim().to_ascii_lowercase().as_str() {
            "normal" => Self::Normal,
            "stretch" => Self::Stretch,
            "centered" => Self::Centered,
            _ => Self::Contain,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Crop {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub feather: f32,
}

#[derive(Debug, Clone)]
pub struct ImageFrame {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub enum Paint {
    Text {
        color: [u8; 4],
    },
    Shape {
        kind: ObjectKind,
        fill: Box<Fill>,
        stroke: Box<Stroke>,
        radius: f32,
        picture: Option<ImageFrame>,
    },
    Image {
        frames: Vec<ImageFrame>,
        size_mode: SizeMode,
    },
    Empty,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub key: String,
    pub name: String,
    pub kind: ObjectKind,
    pub names: Vec<String>,
    pub authored: Rect,
    pub rect: Rect,
    pub rotation: f32,
    pub opacity: f32,
    pub visible: bool,
    pub paint: Paint,
    pub effects: ObjectEffects,
    pub style: TextStyle,
    pub font: FontChoice,
    pub text: String,
    pub bounding_of: Option<String>,
    pub padding: (f32, f32, f32, f32),
    pub auto_size: AutoSize,
    pub ticker: Option<Ticker>,
    pub sequence_duration: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoSize {
    Fixed,
    Grow,
    Shrink,
}

impl AutoSize {
    pub fn parse(raw: Option<&str>) -> Self {
        match raw.unwrap_or("").trim().to_ascii_lowercase().as_str() {
            "grow" => Self::Grow,
            "shrink" => Self::Shrink,
            _ => Self::Fixed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ticker {
    pub speed: f32,
    pub direction: Direction,
}

#[derive(Debug, Clone)]
pub struct Scene {
    pub width: f32,
    pub height: f32,
    pub nodes: Vec<Node>,
    pub storyboards: Vec<Storyboard>,
    pub warnings: Vec<gt_core::Warning>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextField {
    pub key: String,
    pub name: String,
    pub text: String,
    pub font: FontChoice,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Raster {
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub revision: u64,
    pub content_x: u32,
    pub content_y: u32,
    pub content_w: u32,
    pub content_h: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blend {
    Normal,
    Additive,
    Replace,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DrawSprite {
    pub raster_id: String,
    pub x: f32,
    pub y: f32,
    pub src_x: u32,
    pub src_y: u32,
    pub src_w: u32,
    pub src_h: u32,
    pub opacity: f32,
    pub rotation_deg: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub center_x: f32,
    pub center_y: f32,
    pub blend: Blend,
}
