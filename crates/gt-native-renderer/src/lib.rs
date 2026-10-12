//! CPU scene graph, font resolution, and rasterizer for GT titles.
//!
//! The OBS plugin composites the returned sprites. This crate does not link libobs.

mod composite;
mod error;
mod font;
mod layout;
mod load;
mod model;
mod play;
mod raster;

pub use error::{Error, Result};
pub use font::FontCatalog;
pub use model::{Blend, DrawSprite, FontChoice, Raster, Scene, StoryKind, Storyboard, TextField};
pub use play::{Transport, easing};

use std::collections::BTreeMap;
use std::path::Path;

use model::Node;

/// A loaded title with editable text, font overrides, rasters, and playback.
pub struct Session {
    scene: Scene,
    fonts: FontCatalog,
    rasters: Vec<Raster>,
    playback: play::Playback,
    font_warnings: Vec<gt_core::Warning>,
}

impl Session {
    pub fn load_xml(xml: &str, fonts: FontCatalog) -> Result<Self> {
        Ok(Self::assemble(load::load_xml(xml)?, fonts))
    }

    pub fn load_path_blocking(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self::assemble(
            load::load_path_blocking(path)?,
            FontCatalog::system(),
        ))
    }

    pub fn load_zip_bytes(bytes: &[u8], fonts: FontCatalog) -> Result<Self> {
        let package = gt_core::package::Package::from_zip_bytes(
            Path::new("title.gtzip").to_path_buf(),
            bytes,
        )?;
        let document = gt_core::parse::parse_document(&package.document_xml)?;
        Ok(Self::assemble(
            load::scene_from_package(&package, document)?,
            fonts,
        ))
    }

    fn assemble(mut scene: Scene, mut fonts: FontCatalog) -> Self {
        layout::layout(&mut scene, Some(&mut fonts));
        let (rasters, font_warnings) = raster::rasterize(&scene, &mut fonts);
        let playback = play::Playback::new(&scene, &rasters);
        Self {
            scene,
            fonts,
            rasters,
            playback,
            font_warnings,
        }
    }

    pub fn size(&self) -> (u32, u32) {
        (
            self.scene.width.round().max(1.0) as u32,
            self.scene.height.round().max(1.0) as u32,
        )
    }

    pub fn warnings(&self) -> Vec<gt_core::Warning> {
        let mut warnings = self.scene.warnings.clone();
        warnings.extend(self.font_warnings.clone());
        warnings
    }

    pub fn storyboards(&self) -> &[Storyboard] {
        &self.scene.storyboards
    }

    pub fn fields(&self) -> Vec<TextField> {
        self.scene
            .nodes
            .iter()
            .filter(|node| is_text(node))
            .map(|node| TextField {
                key: node.key.clone(),
                name: node.name.clone(),
                text: node.text.clone(),
                font: node.font.clone(),
            })
            .collect()
    }

    pub fn apply_text(&mut self, key: &str, text: &str) -> bool {
        let Some(node) = self.scene.nodes.iter_mut().find(|node| node.key == key) else {
            return false;
        };
        if node.text == text {
            return false;
        }
        node.text = text.to_string();
        self.relayout();
        true
    }

    pub fn edit_text(&mut self, key: &str, text: &str) -> bool {
        let current = self
            .scene
            .nodes
            .iter()
            .find(|node| node.key == key)
            .map(|node| node.text.clone());
        let Some(current) = current else {
            return false;
        };
        if current == text || self.playback.pending_matches(key, text) {
            return false;
        }
        let apply_now = self
            .playback
            .begin_text_change(&self.scene, key, text.to_string());
        if apply_now {
            self.apply_text(key, text);
        } else {
            self.playback.tick(&mut self.scene, &self.rasters, 0.0);
        }
        true
    }

    pub fn apply_font(&mut self, key: &str, font: FontChoice) -> bool {
        let Some(node) = self.scene.nodes.iter_mut().find(|node| node.key == key) else {
            return false;
        };
        if node.font == font {
            return false;
        }
        node.font = font;
        self.relayout();
        true
    }

    pub fn tick(&mut self, seconds: f32) {
        if self.playback.tick(&mut self.scene, &self.rasters, seconds) {
            self.relayout();
        }
    }

    pub fn play_in(&mut self) {
        self.playback
            .play_kind(&self.scene, &self.rasters, StoryKind::TransitionIn);
    }

    pub fn play_out(&mut self) {
        self.playback
            .play_kind(&self.scene, &self.rasters, StoryKind::TransitionOut);
    }

    pub fn play_storyboard(&mut self, index: usize) {
        self.playback.play_index(&self.scene, &self.rasters, index);
    }

    pub fn select_storyboard(&mut self, index: Option<usize>) {
        self.playback.select(index);
    }

    pub fn restart_selected(&mut self) {
        self.playback.play_selected(&self.scene, &self.rasters);
    }

    pub fn pause(&mut self, paused: bool) {
        self.playback.pause(paused);
    }

    pub fn seek(&mut self, seconds: f32) {
        self.playback.seek(&self.scene, &self.rasters, seconds);
    }

    pub fn elapsed(&self) -> f32 {
        self.playback.elapsed()
    }

    pub fn selected_storyboard(&self) -> Option<usize> {
        self.playback.selected()
    }

    pub fn duration(&self) -> f32 {
        self.playback.duration(&self.scene)
    }

    pub fn transport(&self) -> Transport {
        self.playback.transport(&self.scene)
    }

    pub fn sprites(&self) -> &[DrawSprite] {
        &self.playback.sprites
    }

    pub fn rasters(&self) -> &[Raster] {
        &self.rasters
    }

    pub fn flatten_png(&self) -> Vec<u8> {
        let (width, height) = self.size();
        composite::flatten_png(width, height, &self.rasters, self.sprites())
    }

    pub fn node(&self, key: &str) -> Option<&Node> {
        self.scene.nodes.iter().find(|node| node.key == key)
    }

    fn relayout(&mut self) {
        layout::layout(&mut self.scene, Some(&mut self.fonts));
        let (rasters, font_warnings) = raster::rasterize(&self.scene, &mut self.fonts);
        self.rasters = rasters;
        self.font_warnings = font_warnings;
        self.playback.tick(&mut self.scene, &self.rasters, 0.0);
    }
}

fn is_text(node: &Node) -> bool {
    matches!(node.paint, model::Paint::Text { .. })
}

pub fn load_xml_with_fonts(xml: &str, font_bytes: &[u8]) -> Result<Session> {
    Session::load_xml(xml, FontCatalog::from_font_bytes(font_bytes))
}

pub fn setting_key(prefix: &str, key: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in key.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{prefix}_{hash:08x}")
}

pub fn storyboard_choices(scene_storyboards: &[Storyboard]) -> BTreeMap<String, usize> {
    scene_storyboards
        .iter()
        .enumerate()
        .map(|(index, storyboard)| (storyboard.id.clone(), index))
        .collect()
}
