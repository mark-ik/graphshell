use crate::registries::atomic::viewer::{
    EmbeddedViewer, EmbeddedViewerContext, EmbeddedViewerOutput,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::BufReader;

struct AudioState {
    sink: rodio::Sink,
    _stream: rodio::OutputStream,
    loaded_path: String,
    duration: Option<std::time::Duration>,
}

thread_local! {
    static AUDIO_STATES: RefCell<HashMap<crate::graph::NodeKey, AudioState>> =
        RefCell::new(HashMap::new());
}

pub(crate) struct AudioEmbeddedViewer;

impl EmbeddedViewer for AudioEmbeddedViewer {
    fn viewer_id(&self) -> &'static str {
        "viewer:audio"
    }

    fn render(
        &self,
        ui: &mut egui::Ui,
        ctx: &EmbeddedViewerContext<'_>,
    ) -> EmbeddedViewerOutput {
        match render_audio(ui, ctx) {
            Ok(()) => {}
            Err(err) => {
                ui.colored_label(egui::Color32::from_rgb(220, 80, 80), err);
            }
        }
        EmbeddedViewerOutput::empty()
    }
}

#[derive(Clone, Copy)]
enum AudioAction {
    None,
    Play,
    Pause,
    Stop,
    Reload,
    SetVolume(f32),
}

fn render_audio(ui: &mut egui::Ui, ctx: &EmbeddedViewerContext<'_>) -> Result<(), String> {
    let path =
        crate::shell::desktop::workbench::tile_behavior::guarded_file_path_from_node_url(
            ctx.node_url,
            ctx.file_access_policy,
        )?;
    let path_str = path.to_string_lossy().to_string();
    let node_key = ctx.node_key;

    // Phase 1 — ensure audio state exists for this node.
    let needs_init = AUDIO_STATES.with(|states| {
        let states = states.borrow();
        match states.get(&node_key) {
            Some(state) => state.loaded_path != path_str,
            None => true,
        }
    });

    if needs_init {
        let duration = probe_duration(&path);

        let (stream, stream_handle) = rodio::OutputStream::try_default()
            .map_err(|e| format!("Audio output error: {e}"))?;
        let sink = rodio::Sink::try_new(&stream_handle)
            .map_err(|e| format!("Audio sink error: {e}"))?;
        sink.pause();

        let file = std::fs::File::open(&path)
            .map_err(|e| format!("Failed to open '{}': {e}", path.display()))?;
        let reader = BufReader::new(file);
        let source = rodio::Decoder::new(reader)
            .map_err(|e| format!("Failed to decode audio: {e}"))?;
        sink.append(source);

        AUDIO_STATES.with(|states| {
            states.borrow_mut().insert(
                node_key,
                AudioState {
                    sink,
                    _stream: stream,
                    loaded_path: path_str.clone(),
                    duration,
                },
            );
        });
    }

    // Phase 2 — read playback state for UI rendering.
    let (is_paused, is_empty, volume, duration) = AUDIO_STATES.with(|states| {
        let states = states.borrow();
        match states.get(&node_key) {
            Some(s) => (
                s.sink.is_paused(),
                s.sink.empty(),
                s.sink.volume(),
                s.duration,
            ),
            None => (true, true, 1.0, None),
        }
    });

    // Phase 3 — draw transport controls.
    let file_name = path_str
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(&path_str);

    let duration_label = duration
        .map(|d| {
            let secs = d.as_secs();
            format!("{}:{:02}", secs / 60, secs % 60)
        })
        .unwrap_or_else(|| "—".to_string());

    ui.label(format!("🔊 {file_name}  [{duration_label}]"));

    let mut action = AudioAction::None;
    let mut new_volume = volume;

    ui.horizontal(|ui| {
        if is_empty {
            ui.label("Playback complete.");
            if ui.button("⟲ Reload").clicked() {
                action = AudioAction::Reload;
            }
        } else if is_paused {
            if ui.button("▶ Play").clicked() {
                action = AudioAction::Play;
            }
        } else {
            if ui.button("⏸ Pause").clicked() {
                action = AudioAction::Pause;
            }
        }

        if !is_empty && ui.button("⏹ Stop").clicked() {
            action = AudioAction::Stop;
        }

        if ui
            .add(egui::Slider::new(&mut new_volume, 0.0..=1.0).text("Vol"))
            .changed()
        {
            action = AudioAction::SetVolume(new_volume);
        }
    });

    // Phase 4 — apply user actions.
    AUDIO_STATES.with(|states| {
        let mut states = states.borrow_mut();
        match action {
            AudioAction::None => {}
            AudioAction::Play => {
                if let Some(s) = states.get(&node_key) {
                    s.sink.play();
                }
            }
            AudioAction::Pause => {
                if let Some(s) = states.get(&node_key) {
                    s.sink.pause();
                }
            }
            AudioAction::Stop => {
                if let Some(s) = states.get(&node_key) {
                    s.sink.stop();
                }
                states.remove(&node_key);
            }
            AudioAction::Reload => {
                states.remove(&node_key);
            }
            AudioAction::SetVolume(vol) => {
                if let Some(s) = states.get(&node_key) {
                    s.sink.set_volume(vol);
                }
            }
        }
    });

    Ok(())
}

/// Probe audio file duration using symphonia without fully decoding.
fn probe_duration(path: &std::path::Path) -> Option<std::time::Duration> {
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::probe::Hint;

    let file = std::fs::File::open(path).ok()?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &Default::default(), &Default::default())
        .ok()?;
    let track = probed.format.default_track()?;
    let time_base = track.codec_params.time_base?;
    let n_frames = track.codec_params.n_frames?;
    let time = time_base.calc_time(n_frames);
    Some(
        std::time::Duration::from_secs(time.seconds)
            + std::time::Duration::from_secs_f64(time.frac),
    )
}

