use tiny_skia::{Pixmap, PixmapPaint, Transform};

use crate::model::{Blend, DrawSprite, Raster};

pub fn flatten_png(width: u32, height: u32, rasters: &[Raster], sprites: &[DrawSprite]) -> Vec<u8> {
    let mut canvas = Pixmap::new(width.max(1), height.max(1)).expect("canvas");
    for sprite in sprites {
        let Some(raster) = rasters.iter().find(|raster| raster.id == sprite.raster_id) else {
            continue;
        };
        let Some(mut source) = Pixmap::from_vec(
            raster.rgba.clone(),
            tiny_skia::IntSize::from_wh(raster.width.max(1), raster.height.max(1))
                .expect("raster size"),
        ) else {
            continue;
        };
        if sprite.opacity < 0.999 {
            let data = source.data_mut();
            let opacity = sprite.opacity.clamp(0.0, 1.0);
            for channel in data.as_chunks_mut::<4>().0 {
                for sample in channel.iter_mut() {
                    *sample = (*sample as f32 * opacity).round() as u8;
                }
            }
        }
        let _ = (sprite.blend, Blend::Normal, Transform::identity());
        canvas.draw_pixmap(
            sprite.x.round() as i32,
            sprite.y.round() as i32,
            source.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
    }
    canvas.encode_png().unwrap_or_default()
}
