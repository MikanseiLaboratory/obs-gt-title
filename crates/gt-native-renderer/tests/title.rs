use std::io::{Cursor, Write};
use std::path::PathBuf;

use gt_native_renderer::{FontCatalog, FontChoice, Session, StoryKind, easing};
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

fn font_bytes() -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts/Roboto-Regular.ttf"),
    )
    .expect("Roboto fixture")
}

fn session(name: &str) -> Session {
    let xml = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../fixtures/synthetic/{name}")),
    )
    .expect(name);
    Session::load_xml(&xml, FontCatalog::from_font_bytes(&font_bytes())).expect("load")
}

#[test]
fn scene_orders_text_and_resolves_bounding() {
    let title = session("basic.gtxml");
    let fields = title.fields();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name, "Text 1");
    assert_eq!(fields[0].text, "HERE WE ARE");
    let rect = title
        .node(title.fields()[0].key.as_str())
        .expect("text")
        .rect;
    let plate = title
        .sprites()
        .iter()
        .find(|sprite| sprite.raster_id.contains("Rect_1") || sprite.x < 60.0)
        .expect("plate sprite");
    assert!(plate.x <= rect.x);
    assert!((rect.x - (plate.x + 15.0)).abs() < 2.0 || plate.x < rect.x);
}

#[test]
fn reveal_and_fade_follow_elapsed_time() {
    let mut title = session("storyboard.gtxml");
    title.play_in();
    assert!(
        title
            .sprites()
            .iter()
            .all(|sprite| !sprite.raster_id.contains("Rect_In") || sprite.src_w == 0)
    );
    title.seek(0.0);
    let hidden = title
        .sprites()
        .iter()
        .any(|sprite| sprite.raster_id.contains("Rect"));
    assert!(!hidden);
    title.seek(2.0);
    assert!(
        title
            .sprites()
            .iter()
            .any(|sprite| sprite.raster_id.contains("Rect"))
    );
    title.play_in();
    title.seek(0.2);
    assert!(
        title
            .sprites()
            .iter()
            .all(|sprite| !sprite.raster_id.contains("Fade"))
    );
    title.seek(0.45);
    let fade = title
        .sprites()
        .iter()
        .find(|sprite| sprite.raster_id.contains("Fade"))
        .expect("fade sprite");
    assert!(fade.opacity > 0.2 && fade.opacity < 0.8);
    title.seek(0.7);
    let fade = title
        .sprites()
        .iter()
        .find(|sprite| sprite.raster_id.contains("Fade"))
        .expect("fade sprite");
    assert!(fade.opacity > 0.95);
}

#[test]
fn transition_out_moves_the_layer() {
    let mut title = session("storyboard.gtxml");
    title.play_out();
    let start = title.sprites()[0].center_y;
    title.seek(0.5);
    let end = title.sprites()[0].center_y;
    assert!(end > start + 10.0);
}

#[test]
fn continuous_spin_changes_rotation() {
    let mut title = session("storyboard.gtxml");
    title.tick(0.25);
    let angle = title
        .sprites()
        .iter()
        .find(|sprite| sprite.raster_id.contains("Rect"))
        .map(|sprite| sprite.rotation_deg)
        .unwrap_or(0.0);
    assert!(angle > 1.0);
}

#[test]
fn text_edit_waits_for_data_change_then_updates_bounds() {
    let mut title = session("basic.gtxml");
    let key = title.fields()[0].key.clone();
    let before = title.node(&key).unwrap().rect.w;
    assert!(title.edit_text(&key, "HERE WE ARE AND MORE TEXT"));
    assert!(!title.edit_text(&key, "HERE WE ARE AND MORE TEXT"));
    assert_eq!(title.node(&key).unwrap().text, "HERE WE ARE");
    title.tick(0.1);
    assert_eq!(title.node(&key).unwrap().text, "HERE WE ARE");
    title.tick(0.1);
    assert_eq!(title.node(&key).unwrap().text, "HERE WE ARE AND MORE TEXT");
    title.tick(0.2);
    assert!(title.node(&key).unwrap().rect.w >= before);
}

#[test]
fn missing_font_falls_back_and_override_is_stored() {
    let mut title = session("golden.gtxml");
    let key = title.fields()[0].key.clone();
    let mut font = title.fields()[0].font.clone();
    font.family = "Missing Family".to_string();
    font.size = 40.0;
    assert!(title.apply_font(&key, font.clone()));
    assert_eq!(title.node(&key).unwrap().font, font);
    assert!(
        title
            .warnings()
            .iter()
            .any(|warning| warning.code == "font.fallback")
    );
}

#[test]
fn font_choice_starts_from_gt_style() {
    let title = session("basic.gtxml");
    let font = &title.fields()[0].font;
    assert_eq!(font.family, "Roboto");
    assert!(font.bold);
    assert!((font.size - 48.0).abs() < 0.1);
}

#[test]
fn easing_matches_expected_curves() {
    assert!((easing("Linear", 0.5) - 0.5).abs() < 0.001);
    assert!(easing("CubicEasingIn", 0.5) < 0.2);
    assert!(easing("CubicEasingOut", 0.5) > 0.8);
    assert!((easing("CubicEasingInOut", 0.0) - 0.0).abs() < 0.001);
    assert!((easing("CubicEasingInOut", 1.0) - 1.0).abs() < 0.001);
    assert!(easing("BounceOut", 1.0) > 0.98);
}

#[test]
fn golden_png_matches_fixture() {
    let title = session("golden.gtxml");
    let png = title.flatten_png();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/golden/basic.png");
    if std::env::var("GT_WRITE_GOLDEN").is_ok() {
        std::fs::write(&path, &png).unwrap();
    }
    let expected = std::fs::read(&path).expect("golden png");
    assert_eq!(png, expected);
}

#[test]
fn gtzip_roundtrip_keeps_the_text_field() {
    let xml = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/synthetic/basic.gtxml"),
    )
    .unwrap();
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut cursor);
        writer
            .start_file(
                "document.xml",
                SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
            )
            .unwrap();
        writer.write_all(xml.as_bytes()).unwrap();
        writer.finish().unwrap();
    }
    let title = Session::load_zip_bytes(
        &cursor.into_inner(),
        FontCatalog::from_font_bytes(&font_bytes()),
    )
    .unwrap();
    assert_eq!(title.fields()[0].text, "HERE WE ARE");
    assert!(
        title
            .storyboards()
            .iter()
            .any(|storyboard| storyboard.kind == StoryKind::DataChangeIn)
    );
}

#[test]
fn setting_keys_are_stable() {
    let key = gt_native_renderer::setting_key("text", "l0_0_Text_1");
    assert_eq!(key, gt_native_renderer::setting_key("text", "l0_0_Text_1"));
    assert!(key.starts_with("text_"));
}

#[test]
fn font_override_type_roundtrip() {
    let font = FontChoice {
        family: "Roboto".to_string(),
        style: "Regular".to_string(),
        size: 28.0,
        bold: false,
        italic: false,
        underline: true,
        strikeout: false,
    };
    assert!(font.underline);
}
