use crate::load::blend_of;
use crate::model::{
    AnimKind, Animation, Blend, Direction, DrawSprite, Easing, Node, Raster, Scene, StoryKind,
    Storyboard,
};

#[derive(Debug, Clone)]
struct Active {
    index: usize,
    elapsed: f32,
}

#[derive(Debug, Clone)]
enum ChangePhase {
    Out,
    In,
}

#[derive(Debug, Clone)]
struct Change {
    phase: ChangePhase,
    elapsed: f32,
    pending: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct Clock {
    primary: Option<Active>,
    continuous_elapsed: f32,
    change: Option<Change>,
    pub paused: bool,
    selected: Option<usize>,
}

impl Clock {
    fn new(scene: &Scene) -> Self {
        let selected = scene
            .storyboards
            .iter()
            .position(|storyboard| storyboard.kind == StoryKind::TransitionIn)
            .or(scene.storyboards.first().map(|_| 0));
        let mut clock = Self {
            primary: None,
            continuous_elapsed: 0.0,
            change: None,
            paused: false,
            selected,
        };
        if let Some(index) = selected {
            let duration = scene.storyboards[index].duration();
            clock.primary = Some(Active {
                index,
                elapsed: duration,
            });
        }
        clock
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    Playing,
    Paused,
    Ended,
    Stopped,
}

pub struct Playback {
    clock: Clock,
    pub sprites: Vec<DrawSprite>,
}

impl Playback {
    pub fn new(scene: &Scene, rasters: &[Raster]) -> Self {
        let clock = Clock::new(scene);
        let sprites = build_sprites(scene, rasters, &clock);
        Self { clock, sprites }
    }

    pub fn tick(&mut self, scene: &mut Scene, rasters: &[Raster], dt: f32) -> bool {
        if !self.clock.paused {
            self.clock.continuous_elapsed += dt;
            if let Some(primary) = &mut self.clock.primary {
                primary.elapsed += dt;
            }
            if let Some(change) = &mut self.clock.change {
                change.elapsed += dt;
            }
        }
        let mut changed = false;
        if let Some(change) = &self.clock.change
            && matches!(change.phase, ChangePhase::Out)
        {
            let duration = storyboard_duration(scene, StoryKind::DataChangeOut);
            if change.elapsed >= duration {
                let pending = change.pending.clone();
                for (key, text) in pending {
                    if let Some(node) = scene.nodes.iter_mut().find(|node| node.key == key) {
                        node.text = text;
                    }
                }
                changed = true;
                if storyboard_index(scene, StoryKind::DataChangeIn).is_some() {
                    self.clock.change = Some(Change {
                        phase: ChangePhase::In,
                        elapsed: 0.0,
                        pending: Vec::new(),
                    });
                } else {
                    self.clock.change = None;
                }
            }
        } else if let Some(change) = &self.clock.change
            && matches!(change.phase, ChangePhase::In)
        {
            let duration = storyboard_duration(scene, StoryKind::DataChangeIn);
            if change.elapsed >= duration {
                self.clock.change = None;
            }
        }
        self.sprites = build_sprites(scene, rasters, &self.clock);
        changed
    }

    pub fn play_index(&mut self, scene: &Scene, rasters: &[Raster], index: usize) {
        if index >= scene.storyboards.len() {
            return;
        }
        self.clock.selected = Some(index);
        self.clock.primary = Some(Active {
            index,
            elapsed: 0.0,
        });
        self.clock.continuous_elapsed = 0.0;
        self.clock.paused = false;
        self.sprites = build_sprites(scene, rasters, &self.clock);
    }

    pub fn play_kind(&mut self, scene: &Scene, rasters: &[Raster], kind: StoryKind) {
        if let Some(index) = storyboard_index(scene, kind) {
            self.play_index(scene, rasters, index);
        }
    }

    pub fn play_selected(&mut self, scene: &Scene, rasters: &[Raster]) {
        if let Some(index) = self.clock.selected {
            self.play_index(scene, rasters, index);
        }
    }

    pub fn select(&mut self, index: Option<usize>) {
        self.clock.selected = index;
    }

    pub fn selected(&self) -> Option<usize> {
        self.clock.selected
    }

    pub fn pending_matches(&self, key: &str, text: &str) -> bool {
        self.clock.change.as_ref().is_some_and(|change| {
            matches!(change.phase, ChangePhase::Out)
                && change
                    .pending
                    .iter()
                    .any(|(pending_key, pending_text)| pending_key == key && pending_text == text)
        })
    }

    pub fn begin_text_change(&mut self, scene: &Scene, key: &str, text: String) -> bool {
        let has_out = storyboard_index(scene, StoryKind::DataChangeOut).is_some();
        let has_in = storyboard_index(scene, StoryKind::DataChangeIn).is_some();
        if has_out {
            if let Some(change) = &mut self.clock.change
                && matches!(change.phase, ChangePhase::Out)
            {
                if let Some(entry) = change
                    .pending
                    .iter_mut()
                    .find(|(pending_key, _)| pending_key == key)
                {
                    entry.1 = text;
                } else {
                    change.pending.push((key.to_string(), text));
                }
                return false;
            }
            self.clock.change = Some(Change {
                phase: ChangePhase::Out,
                elapsed: 0.0,
                pending: vec![(key.to_string(), text)],
            });
            false
        } else if has_in {
            self.clock.change = Some(Change {
                phase: ChangePhase::In,
                elapsed: 0.0,
                pending: Vec::new(),
            });
            true
        } else {
            true
        }
    }

    pub fn pause(&mut self, paused: bool) {
        self.clock.paused = paused;
    }

    pub fn seek(&mut self, scene: &Scene, rasters: &[Raster], seconds: f32) {
        if let Some(primary) = &mut self.clock.primary {
            primary.elapsed = seconds.max(0.0);
        }
        self.sprites = build_sprites(scene, rasters, &self.clock);
    }

    pub fn elapsed(&self) -> f32 {
        self.clock
            .change
            .as_ref()
            .map(|change| change.elapsed)
            .or_else(|| self.clock.primary.as_ref().map(|primary| primary.elapsed))
            .unwrap_or(0.0)
    }

    pub fn duration(&self, scene: &Scene) -> f32 {
        if let Some(change) = &self.clock.change {
            let kind = match change.phase {
                ChangePhase::Out => StoryKind::DataChangeOut,
                ChangePhase::In => StoryKind::DataChangeIn,
            };
            return storyboard_duration(scene, kind);
        }
        self.clock
            .primary
            .as_ref()
            .and_then(|primary| scene.storyboards.get(primary.index))
            .map(Storyboard::duration)
            .unwrap_or(0.0)
    }

    pub fn transport(&self, scene: &Scene) -> Transport {
        if self.clock.paused {
            return Transport::Paused;
        }
        if self.clock.change.is_some() {
            return Transport::Playing;
        }
        let Some(primary) = &self.clock.primary else {
            return Transport::Stopped;
        };
        let duration = scene
            .storyboards
            .get(primary.index)
            .map(Storyboard::duration)
            .unwrap_or(0.0);
        if primary.elapsed + 0.0001 < duration {
            Transport::Playing
        } else if scene
            .storyboards
            .get(primary.index)
            .is_some_and(|storyboard| storyboard.kind == StoryKind::TransitionOut)
        {
            Transport::Stopped
        } else {
            Transport::Ended
        }
    }
}

fn storyboard_index(scene: &Scene, kind: StoryKind) -> Option<usize> {
    scene
        .storyboards
        .iter()
        .position(|storyboard| storyboard.kind == kind)
}

fn storyboard_duration(scene: &Scene, kind: StoryKind) -> f32 {
    storyboard_index(scene, kind)
        .map(|index| scene.storyboards[index].duration())
        .unwrap_or(0.0)
}

fn build_sprites(scene: &Scene, rasters: &[Raster], clock: &Clock) -> Vec<DrawSprite> {
    let mut sprites = Vec::new();
    for node in &scene.nodes {
        if !node.visible {
            continue;
        }
        let raster = pick_raster(node, rasters, clock, scene);
        let Some(raster) = raster else {
            continue;
        };
        let mut transform = Transform::identity();
        apply_storyboards(scene, clock, node, &mut transform);
        let opacity = (node.opacity * transform.opacity).clamp(0.0, 1.0);
        if opacity <= 0.001 {
            continue;
        }
        let (src_x, src_y, src_w, src_h, x, y) = source_rect(node, raster, &transform, clock);
        if src_w == 0 || src_h == 0 {
            continue;
        }
        sprites.push(DrawSprite {
            raster_id: raster.id.clone(),
            x,
            y,
            src_x,
            src_y,
            src_w,
            src_h,
            opacity,
            rotation_deg: node.rotation + transform.rotation,
            scale_x: transform.scale_x * if node.effects.flip_x { -1.0 } else { 1.0 },
            scale_y: transform.scale_y * if node.effects.flip_y { -1.0 } else { 1.0 },
            center_x: node.rect.x + transform.tx + node.rect.w * 0.5,
            center_y: node.rect.y + transform.ty + node.rect.h * 0.5,
            blend: match blend_of(node) {
                Blend::Additive => Blend::Additive,
                Blend::Replace => Blend::Replace,
                Blend::Normal => Blend::Normal,
            },
        });
    }
    sprites
}

fn pick_raster<'a>(
    node: &Node,
    rasters: &'a [Raster],
    clock: &Clock,
    scene: &Scene,
) -> Option<&'a Raster> {
    let frames: Vec<&Raster> = rasters
        .iter()
        .filter(|raster| raster.id == node.key || raster.id.starts_with(&format!("{}#", node.key)))
        .collect();
    if frames.is_empty() {
        return None;
    }
    if frames.len() == 1 {
        return frames.into_iter().next();
    }
    let duration = node.sequence_duration.max(0.05);
    let elapsed = clock.continuous_elapsed
        + clock
            .primary
            .as_ref()
            .map(|primary| primary.elapsed)
            .unwrap_or(0.0);
    let progress = (elapsed / duration).fract();
    let index = ((progress * frames.len() as f32) as usize).min(frames.len() - 1);
    let _ = scene;
    frames.into_iter().nth(index)
}

#[derive(Clone, Copy)]
struct Transform {
    opacity: f32,
    tx: f32,
    ty: f32,
    scale_x: f32,
    scale_y: f32,
    rotation: f32,
    clip_l: f32,
    clip_t: f32,
    clip_r: f32,
    clip_b: f32,
}

impl Transform {
    fn identity() -> Self {
        Self {
            opacity: 1.0,
            tx: 0.0,
            ty: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
            clip_l: 0.0,
            clip_t: 0.0,
            clip_r: 0.0,
            clip_b: 0.0,
        }
    }
}

fn apply_storyboards(scene: &Scene, clock: &Clock, node: &Node, transform: &mut Transform) {
    if let Some(primary) = &clock.primary
        && let Some(storyboard) = scene.storyboards.get(primary.index)
        && storyboard.kind != StoryKind::Continuous
    {
        apply_storyboard(storyboard, primary.elapsed, false, node, transform);
    }
    for storyboard in scene
        .storyboards
        .iter()
        .filter(|storyboard| storyboard.kind == StoryKind::Continuous)
    {
        apply_storyboard(storyboard, clock.continuous_elapsed, true, node, transform);
    }
    if let Some(change) = &clock.change {
        let kind = match change.phase {
            ChangePhase::Out => StoryKind::DataChangeOut,
            ChangePhase::In => StoryKind::DataChangeIn,
        };
        if let Some(index) = storyboard_index(scene, kind) {
            apply_storyboard(
                &scene.storyboards[index],
                change.elapsed,
                false,
                node,
                transform,
            );
        }
    }
}

fn apply_storyboard(
    storyboard: &Storyboard,
    elapsed: f32,
    looping: bool,
    node: &Node,
    transform: &mut Transform,
) {
    for animation in storyboard
        .animations
        .iter()
        .filter(|animation| node.names.iter().any(|name| name == &animation.target))
        .take(3)
    {
        apply_animation(
            storyboard.kind,
            animation,
            elapsed,
            looping,
            node,
            transform,
        );
    }
}

fn apply_animation(
    kind: StoryKind,
    animation: &Animation,
    elapsed: f32,
    looping: bool,
    node: &Node,
    transform: &mut Transform,
) {
    if matches!(
        animation.kind,
        AnimKind::None | AnimKind::ImageSequence | AnimKind::Other
    ) {
        return;
    }
    let duration = animation.duration.max(0.001);
    let local = elapsed - animation.delay;
    if local < 0.0 && !looping {
        apply_progress(kind, animation, 0.0, node, transform);
        return;
    }
    let linear = if looping {
        (local.max(0.0) / duration).fract()
    } else {
        (local / duration).clamp(0.0, 1.0)
    };
    let eased = animation.easing.apply(linear);
    let reverse = animation.reverse ^ kind.exits();
    let progress = if reverse { 1.0 - eased } else { eased };
    apply_progress(kind, animation, progress, node, transform);
}

fn apply_progress(
    _kind: StoryKind,
    animation: &Animation,
    progress: f32,
    node: &Node,
    transform: &mut Transform,
) {
    match animation.kind {
        AnimKind::Fade => transform.opacity *= progress,
        AnimKind::Reveal | AnimKind::Wipe => {
            let hidden = 1.0 - progress;
            match (animation.direction, animation.center_axis) {
                (Direction::Right, _) => transform.clip_l = hidden,
                (Direction::Top, _) => transform.clip_b = hidden,
                (Direction::Bottom, _) => transform.clip_t = hidden,
                (Direction::Center, crate::model::Axis::Y) => {
                    transform.clip_t = hidden * 0.5;
                    transform.clip_b = hidden * 0.5;
                }
                (Direction::Center, _) => {
                    transform.clip_l = hidden * 0.5;
                    transform.clip_r = hidden * 0.5;
                }
                (Direction::Left, _) => transform.clip_r = hidden,
            }
        }
        AnimKind::Fly => {
            let distance = 1.1 * (1.0 - progress);
            match animation.direction {
                Direction::Right => transform.tx += node.rect.w * distance,
                Direction::Top => transform.ty -= node.rect.h * distance,
                Direction::Bottom => transform.ty += node.rect.h * distance,
                Direction::Center | Direction::Left => transform.tx -= node.rect.w * distance,
            }
        }
        AnimKind::ZoomFade => {
            transform.opacity *= progress;
            let scale = 0.85 + 0.15 * progress;
            transform.scale_x *= scale;
            transform.scale_y *= scale;
        }
        AnimKind::Scale => {
            transform.scale_x *= progress;
            transform.scale_y *= progress;
        }
        AnimKind::Rotate => transform.rotation += -90.0 * (1.0 - progress),
        AnimKind::Spin => transform.rotation += 360.0 * progress,
        AnimKind::None | AnimKind::ImageSequence | AnimKind::Other => {}
    }
}

fn source_rect(
    node: &Node,
    raster: &Raster,
    transform: &Transform,
    clock: &Clock,
) -> (u32, u32, u32, u32, f32, f32) {
    let mut src_x = raster.content_x;
    let mut src_y = raster.content_y;
    let mut src_w = raster.content_w.min(raster.width.saturating_sub(src_x));
    let mut src_h = raster.content_h.min(raster.height.saturating_sub(src_y));
    let mut x = node.rect.x + transform.tx - raster.content_x as f32;
    let mut y = node.rect.y + transform.ty - raster.content_y as f32;
    if let Some(ticker) = node.ticker {
        let period = raster.content_w.max(1) as f32;
        let speed = ticker.speed * 30.0;
        let offset = (clock.continuous_elapsed * speed) % period;
        let viewport = node.rect.w.round().max(1.0) as u32;
        match ticker.direction {
            Direction::Right => {
                src_x = raster.content_x + ((period - offset) as u32) % raster.content_w.max(1);
            }
            Direction::Top | Direction::Bottom => {
                src_y = raster.content_y + (offset as u32) % raster.content_h.max(1);
            }
            Direction::Left | Direction::Center => {
                src_x = raster.content_x + (offset as u32) % raster.content_w.max(1);
            }
        }
        src_w = viewport.min(raster.width.saturating_sub(src_x));
        src_h = node.rect.h.round().max(1.0) as u32;
        x = node.rect.x + transform.tx;
        y = node.rect.y + transform.ty;
    }
    let left = transform.clip_l.clamp(0.0, 1.0);
    let right = transform.clip_r.clamp(0.0, 1.0);
    let top = transform.clip_t.clamp(0.0, 1.0);
    let bottom = transform.clip_b.clamp(0.0, 1.0);
    let visible_w = (1.0 - left - right).clamp(0.0, 1.0);
    let visible_h = (1.0 - top - bottom).clamp(0.0, 1.0);
    x += src_w as f32 * left;
    y += src_h as f32 * top;
    src_x += (src_w as f32 * left) as u32;
    src_y += (src_h as f32 * top) as u32;
    src_w = (src_w as f32 * visible_w) as u32;
    src_h = (src_h as f32 * visible_h) as u32;
    (src_x, src_y, src_w, src_h, x, y)
}

pub fn easing(name: &str, t: f32) -> f32 {
    Easing::parse(Some(name)).apply(t)
}
