//! OBS Studio input source that composites GT titles on the GPU.
//!
//! File loading runs on a worker thread. Raster uploads and draws stay on the
//! graphics thread, which is where OBS requires texture lifetime to be managed.

use std::ffi::{CStr, CString};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, MutexGuard};
use std::thread;

use gt_native_renderer::{
    Blend, DrawSprite, FontChoice, Raster, Session, StoryKind, Transport, setting_key,
};
use obs_wrapper::graphics::{GraphicsColorFormat, GraphicsTexture};
use obs_wrapper::log::Logger;
use obs_wrapper::media::state::MediaState;
use obs_wrapper::obs_register_module;
use obs_wrapper::obs_string;
use obs_wrapper::obs_sys::{
    self, OBS_FONT_BOLD, OBS_FONT_ITALIC, OBS_FONT_STRIKEOUT, OBS_FONT_UNDERLINE,
    OBS_SOURCE_CUSTOM_DRAW, bfree, gs_blend_function, gs_blend_type_GS_BLEND_INVSRCALPHA,
    gs_blend_type_GS_BLEND_ONE, gs_blend_type_GS_BLEND_ZERO, gs_draw_sprite_subregion,
    gs_effect_create, gs_effect_destroy, gs_effect_get_param_by_name, gs_effect_loop,
    gs_effect_set_texture, gs_effect_set_vec4, gs_matrix_pop, gs_matrix_push, gs_matrix_rotaa4f,
    gs_matrix_scale3f, gs_matrix_translate3f, gs_reset_blend_state, obs_data_set_obj,
    obs_enter_graphics, obs_leave_graphics, obs_source_get_settings, obs_source_update_properties,
};
use obs_wrapper::prelude::*;
use obs_wrapper::properties::{
    BoolProp, FontProp, PathProp, PathType, Properties, TextProp, TextType,
};
use obs_wrapper::source::*;
use obs_wrapper::wrapper::PtrWrapper;

const EFFECT: &str = r#"
uniform float4x4 ViewProj;
uniform texture2d image;
uniform float4 color;

sampler_state def_sampler {
    Filter   = Linear;
    AddressU = Clamp;
    AddressV = Clamp;
};

struct VertInOut {
    float4 pos : POSITION;
    float2 uv  : TEXCOORD0;
};

VertInOut VSDefault(VertInOut vert_in)
{
    VertInOut vert_out;
    vert_out.pos = mul(float4(vert_in.pos.xyz, 1.0), ViewProj);
    vert_out.uv  = vert_in.uv;
    return vert_out;
}

float4 PSDraw(VertInOut vert_in) : TARGET
{
    return image.Sample(def_sampler, vert_in.uv) * color;
}

technique Draw
{
    pass
    {
        vertex_shader = VSDefault(vert_in);
        pixel_shader  = PSDraw(vert_in);
    }
}
"#;

struct GtTitleModule {
    context: ModuleRef,
}

struct SourceState {
    session: Option<Session>,
    path: String,
    generation: u64,
    storyboard: String,
}

struct LoadRequest {
    generation: u64,
    path: String,
}

enum LoadResult {
    Ready {
        generation: u64,
        session: Box<Session>,
    },
    Failed {
        generation: u64,
        path: String,
        message: String,
    },
}

struct GpuSprite {
    id: String,
    revision: u64,
    texture: GraphicsTexture,
}

struct GtTitleSource {
    source: SourceRef,
    state: Mutex<SourceState>,
    requests: Sender<LoadRequest>,
    results: Receiver<LoadResult>,
    textures: Vec<GpuSprite>,
    effect: *mut obs_sys::gs_effect_t,
    effect_failed: bool,
}

impl Sourceable for GtTitleSource {
    fn get_id() -> ObsString {
        obs_string!("gt_title")
    }

    fn get_type() -> SourceType {
        SourceType::Input
    }

    fn create(create: &mut CreatableSourceContext<Self>, source: SourceRef) -> Self {
        let (requests, request_rx) = mpsc::channel();
        let (result_tx, results) = mpsc::channel();
        if let Err(error) = thread::Builder::new()
            .name("gt-title-load".to_string())
            .spawn(move || load_worker(request_rx, result_tx))
        {
            log::error!("GT Title failed to start the loader thread: {error}");
        }
        let path = read_string(&create.settings, "file").unwrap_or_default();
        let storyboard = read_string(&create.settings, "storyboard").unwrap_or_default();
        let generation = u64::from(!path.is_empty());
        if generation == 1 {
            let _ = requests.send(LoadRequest {
                generation,
                path: path.clone(),
            });
        }
        Self {
            source,
            state: Mutex::new(SourceState {
                session: None,
                path,
                generation,
                storyboard,
            }),
            requests,
            results,
            textures: Vec::new(),
            effect: std::ptr::null_mut(),
            effect_failed: false,
        }
    }
}

impl Drop for GtTitleSource {
    fn drop(&mut self) {
        if self.effect.is_null() {
            return;
        }
        unsafe {
            obs_enter_graphics();
            gs_effect_destroy(self.effect);
            obs_leave_graphics();
            self.effect = std::ptr::null_mut();
        }
    }
}

impl GetNameSource for GtTitleSource {
    fn get_name() -> ObsString {
        obs_string!("GT Title")
    }
}

impl GetWidthSource for GtTitleSource {
    fn get_width(&mut self) -> u32 {
        let state = lock(&self.state);
        state
            .session
            .as_ref()
            .map(|session| session.size().0)
            .unwrap_or(1920)
            .max(1)
    }
}

impl GetHeightSource for GtTitleSource {
    fn get_height(&mut self) -> u32 {
        let state = lock(&self.state);
        state
            .session
            .as_ref()
            .map(|session| session.size().1)
            .unwrap_or(1080)
            .max(1)
    }
}

impl GetDefaultsSource for GtTitleSource {
    fn get_defaults(settings: &mut DataObj) {
        settings.set_default::<ObsString>("file", "");
        settings.set_default::<bool>("reload", false);
        settings.set_default::<bool>("play_out", false);
        settings.set_default::<ObsString>("storyboard", "");
    }
}

impl GetPropertiesSource for GtTitleSource {
    fn get_properties(&mut self) -> Properties {
        let mut properties = Properties::new();
        properties.add(
            obs_string!("file"),
            obs_string!("GT Title ファイル"),
            PathProp::new(PathType::File).with_filter(obs_string!("GT Title (*.gtzip *.gtxml)")),
        );
        properties.add(obs_string!("reload"), obs_string!("再読み込み"), BoolProp);
        let boards_and_fields = {
            let state = lock(&self.state);
            state.session.as_ref().map(|session| {
                (
                    session
                        .storyboards()
                        .iter()
                        .map(|board| (board.id.clone(), board.label.clone(), board.kind))
                        .collect::<Vec<_>>(),
                    session.fields(),
                )
            })
        };
        if let Some((boards, fields)) = boards_and_fields {
            if !boards.is_empty() {
                let mut list = properties.add_list::<ObsString>(
                    obs_string!("storyboard"),
                    obs_string!("ストーリーボード"),
                    false,
                );
                for (id, label, kind) in boards {
                    let shown = if label.is_empty() {
                        id.clone()
                    } else {
                        format!("{label} ({})", kind_name(kind))
                    };
                    list.push(obs_text(&shown), obs_text(&id));
                }
            }
            for field in fields {
                let name = if field.name.is_empty() {
                    field.key.clone()
                } else {
                    field.name.clone()
                };
                properties.add(
                    obs_text(&setting_key("text", &field.key)),
                    obs_text(&format!("{name} (テキスト)")),
                    TextProp::new(TextType::Multiline),
                );
                properties.add(
                    obs_text(&setting_key("font", &field.key)),
                    obs_text(&format!("{name} (フォント)")),
                    FontProp,
                );
            }
        }
        properties.add(
            obs_string!("play_out"),
            obs_string!("TransitionOut を再生"),
            BoolProp,
        );
        properties
    }
}

impl UpdateSource for GtTitleSource {
    fn update(&mut self, settings: &mut DataObj, _context: &mut GlobalContext) {
        let reload = take_bool(settings, "reload");
        let play_out = take_bool(settings, "play_out");
        let path = read_string(settings, "file").unwrap_or_default();
        let storyboard = read_string(settings, "storyboard").unwrap_or_default();
        let mut state = lock(&self.state);
        let path_changed = path != state.path;
        if path_changed {
            state.path = path.clone();
        }
        let storyboard_changed = storyboard != state.storyboard;
        state.storyboard = storyboard;
        if (reload || path_changed) && !path.is_empty() {
            state.generation = state.generation.wrapping_add(1);
            let generation = state.generation;
            drop(state);
            let _ = self.requests.send(LoadRequest { generation, path });
            return;
        }
        let storyboard_id = state.storyboard.clone();
        if let Some(session) = state.session.as_mut() {
            if storyboard_changed {
                select_storyboard(session, &storyboard_id);
            }
            if play_out {
                session.play_out();
            } else if storyboard_changed {
                play_storyboard_id(session, &storyboard_id);
            }
            apply_overrides(session, settings);
        }
    }
}

impl VideoTickSource for GtTitleSource {
    fn video_tick(&mut self, seconds: f32) {
        let mut refresh = false;
        while let Ok(message) = self.results.try_recv() {
            match message {
                LoadResult::Ready {
                    generation,
                    mut session,
                } => {
                    let (current, path, storyboard) = {
                        let state = lock(&self.state);
                        (
                            state.generation,
                            state.path.clone(),
                            state.storyboard.clone(),
                        )
                    };
                    if generation != current {
                        continue;
                    }
                    if let Some(mut settings) = source_settings(&self.source) {
                        select_storyboard(&mut session, &storyboard);
                        apply_overrides(&mut session, &settings);
                        seed_settings(&mut settings, &session);
                    }
                    log_warnings(&path, &session);
                    let (width, height) = session.size();
                    log::info!("GT Title loaded {path} ({width}x{height})");
                    let mut state = lock(&self.state);
                    if state.generation == generation {
                        if state.storyboard.is_empty()
                            && let Some(index) = session.selected_storyboard()
                            && let Some(board) = session.storyboards().get(index)
                        {
                            state.storyboard = board.id.clone();
                        }
                        state.session = Some(*session);
                        refresh = true;
                    }
                }
                LoadResult::Failed {
                    generation,
                    path,
                    message,
                } => {
                    let state = lock(&self.state);
                    if state.generation == generation {
                        log::error!(
                            "GT Title failed to load {path}: {message}. The previous scene is unchanged."
                        );
                    }
                }
            }
        }
        {
            let mut state = lock(&self.state);
            if let Some(session) = state.session.as_mut() {
                session.tick(seconds);
            }
        }
        if refresh {
            unsafe { obs_source_update_properties(self.source.as_ptr_mut()) }
        }
    }
}

impl VideoRenderSource for GtTitleSource {
    fn video_render(&mut self, _context: &mut GlobalContext, _render: &mut VideoRenderContext) {
        ensure_effect(&mut self.effect, &mut self.effect_failed);
        let state = lock(&self.state);
        let Some(session) = state.session.as_ref() else {
            return;
        };
        sync_textures(&mut self.textures, session.rasters());
        draw_sprites(self.effect, &self.textures, session.sprites());
    }
}

impl ShowSource for GtTitleSource {
    fn show(&mut self) {
        let mut state = lock(&self.state);
        if let Some(session) = state.session.as_mut() {
            session.play_in();
        }
    }
}

impl MediaPlayPauseSource for GtTitleSource {
    fn play_pause(&mut self, pause: bool) {
        let mut state = lock(&self.state);
        let Some(session) = state.session.as_mut() else {
            return;
        };
        if pause {
            session.pause(true);
            return;
        }
        session.pause(false);
        if matches!(session.transport(), Transport::Stopped | Transport::Ended) {
            session.play_in();
        }
    }
}

impl MediaRestartSource for GtTitleSource {
    fn restart(&mut self) {
        let mut state = lock(&self.state);
        if let Some(session) = state.session.as_mut() {
            session.play_in();
        }
    }
}

impl MediaStopSource for GtTitleSource {
    fn stop(&mut self) {
        let mut state = lock(&self.state);
        if let Some(session) = state.session.as_mut() {
            session.play_out();
        }
    }
}

impl MediaGetStateSource for GtTitleSource {
    fn get_state(&mut self) -> MediaState {
        let state = lock(&self.state);
        match state.session.as_ref().map(Session::transport) {
            Some(Transport::Playing) => MediaState::Playing,
            Some(Transport::Paused) => MediaState::Paused,
            Some(Transport::Ended) => MediaState::Ended,
            Some(Transport::Stopped) => MediaState::Stopped,
            None => MediaState::None,
        }
    }
}

impl MediaGetDurationSource for GtTitleSource {
    fn get_duration(&mut self) -> i64 {
        let state = lock(&self.state);
        state
            .session
            .as_ref()
            .map(|session| (session.duration() * 1000.0).round() as i64)
            .unwrap_or(0)
    }
}

impl MediaGetTimeSource for GtTitleSource {
    fn get_time(&mut self) -> i64 {
        let state = lock(&self.state);
        state
            .session
            .as_ref()
            .map(|session| (session.elapsed() * 1000.0).round() as i64)
            .unwrap_or(0)
    }
}

impl MediaSetTimeSource for GtTitleSource {
    fn set_time(&mut self, milliseconds: i64) {
        let mut state = lock(&self.state);
        if let Some(session) = state.session.as_mut() {
            session.seek(milliseconds.max(0) as f32 / 1000.0);
        }
    }
}

impl Module for GtTitleModule {
    fn new(context: ModuleRef) -> Self {
        Self { context }
    }

    fn get_ctx(&self) -> &ModuleRef {
        &self.context
    }

    fn load(&mut self, load_context: &mut LoadContext) -> bool {
        let _ = Logger::new().with_promote_debug(true).init();
        let source = load_context
            .create_source_builder::<GtTitleSource>()
            .enable_get_name()
            .enable_get_width()
            .enable_get_height()
            .enable_get_properties()
            .enable_get_defaults()
            .enable_update()
            .enable_video_tick()
            .enable_video_render()
            .enable_show()
            .enable_media_play_pause()
            .enable_media_restart()
            .enable_media_stop()
            .enable_media_get_state()
            .enable_media_get_duration()
            .enable_media_get_time()
            .enable_media_set_time()
            .with_icon(Icon::Text)
            .with_output_flags(OBS_SOURCE_CUSTOM_DRAW)
            .build();
        load_context.register_source(source);
        true
    }

    fn description() -> ObsString {
        obs_string!("vMix GT Title ネイティブ描画プラグイン")
    }

    fn name() -> ObsString {
        obs_string!("GT Title")
    }

    fn author() -> ObsString {
        obs_string!("MikanseiLaboratory")
    }
}

obs_register_module!(GtTitleModule);

fn load_worker(requests: Receiver<LoadRequest>, results: Sender<LoadResult>) {
    while let Ok(request) = requests.recv() {
        let result = match Session::load_path_blocking(&request.path) {
            Ok(session) => LoadResult::Ready {
                generation: request.generation,
                session: Box::new(session),
            },
            Err(error) => LoadResult::Failed {
                generation: request.generation,
                path: request.path,
                message: error.to_string(),
            },
        };
        if results.send(result).is_err() {
            break;
        }
    }
}

fn lock(state: &Mutex<SourceState>) -> MutexGuard<'_, SourceState> {
    state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn read_string(settings: &DataObj, key: &str) -> Option<String> {
    settings
        .get::<ObsString>(key)
        .map(|value| value.as_str().to_string())
}

fn take_bool(settings: &mut DataObj, key: &str) -> bool {
    let enabled = settings.get::<bool>(key).unwrap_or(false);
    if enabled {
        settings.set_bool(key, false);
    }
    enabled
}

fn obs_text(value: &str) -> ObsString {
    ObsString::from(value.replace('\0', ""))
}

fn source_settings(source: &SourceRef) -> Option<DataObj<'_>> {
    unsafe { DataObj::from_raw_unchecked(obs_source_get_settings(source.as_ptr())) }
}

fn set_obj(settings: &mut DataObj, key: &str, object: &DataObj) {
    let key = obs_text(key);
    unsafe { obs_data_set_obj(settings.as_ptr_mut(), key.as_ptr(), object.as_ptr_mut()) }
}

fn apply_overrides(session: &mut Session, settings: &DataObj) {
    let fields = session.fields();
    for field in fields {
        let font_key = setting_key("font", &field.key);
        if let Some(font_data) = settings.get::<DataObj>(font_key.as_str())
            && let Some(choice) = font_choice(&font_data)
        {
            session.apply_font(&field.key, choice);
        }
        let text_key = setting_key("text", &field.key);
        if let Some(text) = read_string(settings, &text_key) {
            session.edit_text(&field.key, &text);
        }
    }
}

fn seed_settings(settings: &mut DataObj, session: &Session) {
    if read_string(settings, "storyboard").is_none()
        && let Some(index) = session.selected_storyboard()
        && let Some(board) = session.storyboards().get(index)
    {
        settings.set_string("storyboard", board.id.as_str());
    }
    for field in session.fields() {
        let text_key = setting_key("text", &field.key);
        if read_string(settings, &text_key).is_none() {
            settings.set_string(text_key.as_str(), field.text.as_str());
        }
        let font_key = setting_key("font", &field.key);
        if settings.get::<DataObj>(font_key.as_str()).is_none() {
            let font = font_object(&field.font);
            set_obj(settings, &font_key, &font);
        }
    }
}

fn select_storyboard(session: &mut Session, id: &str) {
    if id.is_empty() {
        return;
    }
    let index = session
        .storyboards()
        .iter()
        .position(|board| board.id == id);
    session.select_storyboard(index);
}

fn play_storyboard_id(session: &mut Session, id: &str) {
    if id.is_empty() {
        return;
    }
    if let Some(index) = session
        .storyboards()
        .iter()
        .position(|board| board.id == id)
    {
        session.play_storyboard(index);
    }
}

fn font_choice(font: &DataObj) -> Option<FontChoice> {
    let family = read_string(font, "face").unwrap_or_default();
    if family.is_empty() {
        return None;
    }
    let style = read_string(font, "style").unwrap_or_else(|| "Regular".to_string());
    let size = font.get::<i64>("size").unwrap_or(32).max(1) as f32;
    let flags = font.get::<i64>("flags").unwrap_or(0) as u32;
    Some(FontChoice {
        family,
        style,
        size,
        bold: flags & OBS_FONT_BOLD != 0,
        italic: flags & OBS_FONT_ITALIC != 0,
        underline: flags & OBS_FONT_UNDERLINE != 0,
        strikeout: flags & OBS_FONT_STRIKEOUT != 0,
    })
}

fn font_object(font: &FontChoice) -> DataObj<'_> {
    let mut object = DataObj::new();
    object.set_string("face", font.family.as_str());
    object.set_string("style", font.style.as_str());
    object.set_int("size", font.size.round().max(1.0) as i64);
    let mut flags = 0;
    if font.bold {
        flags |= OBS_FONT_BOLD;
    }
    if font.italic {
        flags |= OBS_FONT_ITALIC;
    }
    if font.underline {
        flags |= OBS_FONT_UNDERLINE;
    }
    if font.strikeout {
        flags |= OBS_FONT_STRIKEOUT;
    }
    object.set_int("flags", i64::from(flags));
    object
}

fn log_warnings(path: &str, session: &Session) {
    for warning in session.warnings() {
        let object = warning.object.as_deref().unwrap_or("-");
        log::warn!(
            "GT Title [{path}] {} ({object}): {}",
            warning.code,
            warning.message
        );
    }
}

fn kind_name(kind: StoryKind) -> &'static str {
    match kind {
        StoryKind::TransitionIn => "TransitionIn",
        StoryKind::TransitionOut => "TransitionOut",
        StoryKind::Continuous => "Continuous",
        StoryKind::DataChangeIn => "DataChangeIn",
        StoryKind::DataChangeOut => "DataChangeOut",
        StoryKind::Other => "Other",
    }
}

fn ensure_effect(effect: &mut *mut obs_sys::gs_effect_t, failed: &mut bool) {
    if !effect.is_null() || *failed {
        return;
    }
    let Ok(source) = CString::new(EFFECT) else {
        log::error!("GT Title effect source contains an interior nul");
        *failed = true;
        return;
    };
    let name = c"gt_title.effect";
    let mut error = std::ptr::null_mut();
    let created = unsafe { gs_effect_create(source.as_ptr(), name.as_ptr(), &mut error) };
    if created.is_null() {
        let message = if error.is_null() {
            "unknown effect error".to_string()
        } else {
            unsafe { CStr::from_ptr(error).to_string_lossy().into_owned() }
        };
        log::error!("GT Title effect failed to compile: {message}");
        if !error.is_null() {
            unsafe { bfree(error.cast()) }
        }
        *failed = true;
        return;
    }
    if !error.is_null() {
        unsafe { bfree(error.cast()) }
    }
    *effect = created;
}

fn sync_textures(textures: &mut Vec<GpuSprite>, rasters: &[Raster]) {
    textures.retain(|texture| rasters.iter().any(|raster| raster.id == texture.id));
    for raster in rasters {
        if raster.width == 0 || raster.height == 0 {
            continue;
        }
        let needed = raster.width as usize * raster.height as usize * 4;
        if raster.rgba.len() < needed {
            continue;
        }
        if let Some(existing) = textures.iter_mut().find(|texture| texture.id == raster.id) {
            let same_size = existing.texture.width() == raster.width
                && existing.texture.height() == raster.height;
            if existing.revision == raster.revision && same_size {
                continue;
            }
            if !same_size {
                existing.texture =
                    GraphicsTexture::new(raster.width, raster.height, GraphicsColorFormat::RGBA);
            }
            existing
                .texture
                .set_image(&raster.rgba[..needed], raster.width * 4, false);
            existing.revision = raster.revision;
        } else {
            let mut texture =
                GraphicsTexture::new(raster.width, raster.height, GraphicsColorFormat::RGBA);
            texture.set_image(&raster.rgba[..needed], raster.width * 4, false);
            textures.push(GpuSprite {
                id: raster.id.clone(),
                revision: raster.revision,
                texture,
            });
        }
    }
}

fn draw_sprites(effect: *mut obs_sys::gs_effect_t, textures: &[GpuSprite], sprites: &[DrawSprite]) {
    if effect.is_null() {
        return;
    }
    unsafe {
        let image = gs_effect_get_param_by_name(effect, c"image".as_ptr());
        let color = gs_effect_get_param_by_name(effect, c"color".as_ptr());
        if image.is_null() || color.is_null() {
            return;
        }
        while gs_effect_loop(effect, c"Draw".as_ptr()) {
            for sprite in sprites {
                let Some(texture) = textures
                    .iter()
                    .find(|texture| texture.id == sprite.raster_id)
                else {
                    continue;
                };
                if sprite.src_w == 0 || sprite.src_h == 0 {
                    continue;
                }
                let opacity = sprite.opacity.clamp(0.0, 1.0);
                let tint = color_vec(opacity);
                gs_effect_set_vec4(color, &tint);
                gs_effect_set_texture(image, texture.texture.as_ptr());
                apply_blend(sprite.blend);
                gs_matrix_push();
                gs_matrix_translate3f(sprite.center_x, sprite.center_y, 0.0);
                gs_matrix_rotaa4f(0.0, 0.0, 1.0, sprite.rotation_deg.to_radians());
                gs_matrix_scale3f(sprite.scale_x, sprite.scale_y, 1.0);
                gs_matrix_translate3f(sprite.x - sprite.center_x, sprite.y - sprite.center_y, 0.0);
                gs_draw_sprite_subregion(
                    texture.texture.as_ptr(),
                    0,
                    sprite.src_x,
                    sprite.src_y,
                    sprite.src_w,
                    sprite.src_h,
                );
                gs_matrix_pop();
            }
        }
        gs_reset_blend_state();
    }
}

fn apply_blend(blend: Blend) {
    let (src, dest) = match blend {
        Blend::Additive => (gs_blend_type_GS_BLEND_ONE, gs_blend_type_GS_BLEND_ONE),
        Blend::Replace => (gs_blend_type_GS_BLEND_ONE, gs_blend_type_GS_BLEND_ZERO),
        Blend::Normal => (
            gs_blend_type_GS_BLEND_ONE,
            gs_blend_type_GS_BLEND_INVSRCALPHA,
        ),
    };
    unsafe { gs_blend_function(src, dest) }
}

fn color_vec(opacity: f32) -> obs_sys::vec4 {
    unsafe {
        let mut value: obs_sys::vec4 = std::mem::zeroed();
        value.__bindgen_anon_1.ptr = [opacity, opacity, opacity, opacity];
        value
    }
}
