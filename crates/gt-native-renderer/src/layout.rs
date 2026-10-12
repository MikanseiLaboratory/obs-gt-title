use crate::font::{FontCatalog, measure_text};
use crate::model::{AutoSize, Node, Rect, Scene};

pub fn layout(scene: &mut Scene, fonts: Option<&mut FontCatalog>) {
    for node in &mut scene.nodes {
        if node.bounding_of.is_none() {
            node.rect = node.authored;
        }
    }
    if let Some(fonts) = fonts {
        for node in &mut scene.nodes {
            if !matches!(node.paint, crate::model::Paint::Text { .. }) {
                continue;
            }
            if node.auto_size == AutoSize::Fixed {
                continue;
            }
            let measured = measure_text(
                fonts,
                &node.text,
                &node.font,
                &node.style,
                if node.auto_size == AutoSize::Grow && !wraps(node) {
                    100_000.0
                } else {
                    node.authored.w
                },
            );
            match node.auto_size {
                AutoSize::Grow => {
                    node.rect.w = measured.width.ceil().max(node.authored.w).max(1.0);
                    node.rect.h = measured.height.ceil().max(node.authored.h).max(1.0);
                }
                AutoSize::Shrink => {
                    node.rect.w = measured.width.ceil().clamp(1.0, node.authored.w);
                    node.rect.h = measured.height.ceil().clamp(1.0, node.authored.h);
                }
                AutoSize::Fixed => {}
            }
        }
    }
    for _ in 0..8 {
        let snapshot: Vec<(String, Rect)> = scene
            .nodes
            .iter()
            .map(|node| (node.name.clone(), node.rect))
            .collect();
        for node in &mut scene.nodes {
            let Some(target) = node.bounding_of.clone() else {
                continue;
            };
            let Some((_, rect)) = snapshot.iter().find(|(name, _)| name == &target) else {
                continue;
            };
            let (left, top, right, bottom) = node.padding;
            node.rect = Rect {
                x: rect.x - left,
                y: rect.y - top,
                w: (rect.w + left + right).max(1.0),
                h: (rect.h + top + bottom).max(1.0),
            };
        }
    }
}

fn wraps(node: &Node) -> bool {
    node.style
        .word_wrapping
        .as_deref()
        .is_some_and(|value| value.to_ascii_lowercase().contains("wrap"))
}
