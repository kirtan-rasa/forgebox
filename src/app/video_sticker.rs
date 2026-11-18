use crate::engine::{JobOptions, VideoStickerFit};
use super::theme::*;
use eframe::egui::{self, Color32, FontId, RichText, Sense, Stroke, TextureHandle, Vec2};
use serde_json::Value;
use std::io::{Read, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

const PLAYBACK_MAX_SIDE: u32 = 512;
const PLAYBACK_FPS: u32 = 24;
const STICKER_PREVIEW_FPS: u32 = 30;
const CROP_HANDLE_SIZE: f32 = 22.0;

#[derive(Clone, Copy)]
struct VideoInfo { width: u32, height: u32, duration_ms: u32 }

enum Message {
    Metadata {
        generation: u64,
        path: PathBuf,
        result: Result<VideoInfo, String>,
    },
    Frame {
        generation: u64,
        path: PathBuf,
        time_ms: u32,
        result: Result<egui::ColorImage, String>,
    },
}

enum PlaybackMessage {
    Frame { generation: u64, time_ms: u32, pixels: Vec<u8> },
    Finished { generation: u64 },
    Error { generation: u64, message: String },
}

#[derive(Clone, Copy)]
enum TimelineDrag {
    Start,
    End,
    Playhead,
    Selection { pointer_anchor_ms: u32, start_ms: u32, end_ms: u32 },
}

struct PlaybackControl {
    stop: Arc<AtomicBool>,
    pid: Arc<AtomicI32>,
}

struct PreviewWorker {
    cancelled: Arc<AtomicBool>,
    worker: JoinHandle<()>,
}

impl PreviewWorker {
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    fn is_finished(&self) -> bool {
        self.worker.is_finished()
    }

    fn join(self) {
        let _ = self.worker.join();
    }
}

pub(super) struct VideoStickerEditor {
    path: Option<PathBuf>,
    info: Option<VideoInfo>,
    error: Option<String>,
    frame_error: Option<String>,
    playback_error: Option<String>,
    start_ms: u32,
    end_ms: u32,
    playhead_ms: u32,
    left: u16,
    top: u16,
    zoom: f32,
    texture: Option<TextureHandle>,
    texture_time: Option<u32>,
    playback_texture: Option<TextureHandle>,
    metadata_pending: Option<PathBuf>,
    frame_pending: Option<(PathBuf, u32)>,
    timeline_drag: Option<TimelineDrag>,
    tx: Sender<Message>,
    rx: Receiver<Message>,
    workers: Vec<PreviewWorker>,
    playback_tx: SyncSender<PlaybackMessage>,
    playback_rx: Receiver<PlaybackMessage>,
    playback_control: Option<PlaybackControl>,
    playback_worker: Option<JoinHandle<()>>,
    playback_generation: u64,
    preview_generation: u64,
    is_playing: bool,
    show_sticker_preview: bool,
    playback_is_sticker: bool,
    playback_restart_pending: bool,
}

impl Default for VideoStickerEditor {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        let (playback_tx, playback_rx) = mpsc::sync_channel(2);
        Self {
            path: None,
            info: None,
            error: None,
            frame_error: None,
            playback_error: None,
            start_ms: 0,
            end_ms: 0,
            playhead_ms: 0,
            left: 500,
            top: 500,
            zoom: 1.0,
            texture: None,
            texture_time: None,
            playback_texture: None,
            metadata_pending: None,
            frame_pending: None,
            timeline_drag: None,
            tx,
            rx,
            workers: Vec::new(),
            playback_tx,
            playback_rx,
            playback_control: None,
            playback_worker: None,
            playback_generation: 0,
            preview_generation: 0,
            is_playing: false,
            show_sticker_preview: false,
            playback_is_sticker: false,
            playback_restart_pending: false,
        }
    }
}

impl VideoStickerEditor {
    pub(super) fn set_input(&mut self, path: Option<PathBuf>) {
        if self.path == path { return; }
        self.preview_generation = self.preview_generation.wrapping_add(1);
        let generation = self.preview_generation;
        self.stop_playback();
        for worker in &self.workers {
            worker.cancel();
        }
        self.path = path.clone();
        self.info = None;
        self.error = None;
        self.frame_error = None;
        self.playback_error = None;
        self.start_ms = 0;
        self.end_ms = 0;
        self.playhead_ms = 0;
        self.left = 500;
        self.top = 500;
        self.zoom = 1.0;
        self.texture = None;
        self.texture_time = None;
        self.playback_texture = None;
        self.metadata_pending = None;
        self.frame_pending = None;
        self.timeline_drag = None;
        self.show_sticker_preview = false;
        self.playback_is_sticker = false;
        self.playback_restart_pending = false;
        if let Some(path) = path {
            self.metadata_pending = Some(path.clone());
            let tx = self.tx.clone();
            let cancelled = Arc::new(AtomicBool::new(false));
            let worker_cancelled = cancelled.clone();
            let worker = thread::spawn(move || {
                let result = inspect_video(&path, &worker_cancelled);
                let _ = tx.send(Message::Metadata { generation, path, result });
            });
            self.workers.push(PreviewWorker { cancelled, worker });
        }
    }

    pub(super) fn poll(&mut self, ctx: &egui::Context) {
        while let Ok(message) = self.rx.try_recv() {
            match message {
                Message::Metadata { generation, path, result } => {
                    if generation != self.preview_generation { continue; }
                    if self.metadata_pending.as_ref() == Some(&path) { self.metadata_pending = None; }
                    if self.path.as_ref() != Some(&path) { continue; }
                    match result {
                        Ok(info) if info.duration_ms >= 100 => {
                            self.end_ms = info.duration_ms.min(3_000);
                            self.playhead_ms = 0;
                            self.info = Some(info);
                        }
                        Ok(_) => self.error = Some("The selected video is shorter than 0.1 seconds.".into()),
                        Err(error) => self.error = Some(error),
                    }
                }
                Message::Frame { generation, path, time_ms, result } => {
                    if generation != self.preview_generation { continue; }
                    if self.frame_pending.as_ref() == Some(&(path.clone(), time_ms)) { self.frame_pending = None; }
                    if self.path.as_ref() != Some(&path) || self.playhead_ms != time_ms { continue; }
                    match result {
                        Ok(image) => {
                            self.texture = Some(ctx.load_texture(
                                format!("video-sticker:{}:{time_ms}", path.display()),
                                image,
                                egui::TextureOptions::LINEAR,
                            ));
                            self.texture_time = Some(time_ms);
                            self.frame_error = None;
                        }
                        Err(error) => self.frame_error = Some(error),
                    }
                }
            }
        }

        while let Ok(message) = self.playback_rx.try_recv() {
            match message {
                PlaybackMessage::Frame { generation, time_ms, pixels } if generation == self.playback_generation && self.is_playing => {
                    if let Some(info) = self.info {
                        let (width, height) = if self.playback_is_sticker { (512, 512) } else { playback_dimensions(info) };
                        let image = egui::ColorImage::from_rgba_unmultiplied(
                            [width as usize, height as usize],
                            &pixels,
                        );
                        self.playback_texture = Some(ctx.load_texture(
                            format!("video-sticker-playback-{generation}-{time_ms}"),
                            image,
                            egui::TextureOptions::LINEAR,
                        ));
                        self.playhead_ms = time_ms.min(info.duration_ms.saturating_sub(1));
                    }
                }
                PlaybackMessage::Finished { generation } if generation == self.playback_generation => {
                    self.is_playing = false;
                    if self.playback_is_sticker {
                        self.playhead_ms = self.start_ms;
                        self.playback_restart_pending = true;
                    } else {
                        self.playback_texture = None;
                        if let Some(info) = self.info {
                            self.playhead_ms = info.duration_ms.saturating_sub(1);
                        }
                    }
                }
                PlaybackMessage::Error { generation, message } if generation == self.playback_generation => {
                    self.is_playing = false;
                    self.playback_restart_pending = false;
                    self.playback_texture = None;
                    self.playback_error = Some(message);
                }
                _ => {}
            }
        }

        if self.playback_worker.as_ref().map(JoinHandle::is_finished).unwrap_or(false) {
            if let Some(worker) = self.playback_worker.take() { let _ = worker.join(); }
            self.playback_control = None;
            if self.is_playing {
                self.is_playing = false;
                self.playback_restart_pending = false;
                self.playback_texture = None;
                if let Some(info) = self.info {
                    self.playhead_ms = info.duration_ms.saturating_sub(1);
                }
            }
            let restart_sticker = self.playback_restart_pending && self.show_sticker_preview;
            self.playback_restart_pending = false;
            if restart_sticker {
                self.start_playback();
            }
        }

        let mut index = 0;
        while index < self.workers.len() {
            if self.workers[index].is_finished() {
                let worker = self.workers.swap_remove(index);
                worker.join();
            } else {
                index += 1;
            }
        }

        if !self.is_playing && self.playback_worker.is_none() {
            if let (Some(path), Some(info)) = (self.path.clone(), self.info) {
                if self.frame_pending.is_none() && self.texture_time != Some(self.playhead_ms) {
                    let at = self.playhead_ms.min(info.duration_ms.saturating_sub(1));
                    self.frame_pending = Some((path.clone(), at));
                    let tx = self.tx.clone();
                    let generation = self.preview_generation;
                    let cancelled = Arc::new(AtomicBool::new(false));
                    let worker_cancelled = cancelled.clone();
                    let worker = thread::spawn(move || {
                        let result = preview_frame(&path, at, &worker_cancelled);
                        let _ = tx.send(Message::Frame { generation, path, time_ms: at, result });
                    });
                    self.workers.push(PreviewWorker { cancelled, worker });
                }
            }
        }

        if self.metadata_pending.is_some() || self.frame_pending.is_some() || self.is_playing || self.playback_worker.is_some() {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }

    pub(super) fn job_options(&self) -> Result<JobOptions, String> {
        let info = self.info.ok_or_else(|| "Wait until ForgeBox has analyzed the selected video.".to_string())?;
        if self.end_ms <= self.start_ms || self.end_ms - self.start_ms > 3_000 {
            return Err("Choose a valid segment no longer than three seconds.".into());
        }
        if self.end_ms > info.duration_ms {
            return Err("The selected segment extends beyond the source video.".into());
        }
        Ok(JobOptions::VideoSticker {
            start_ms: self.start_ms,
            end_ms: self.end_ms,
            fit: VideoStickerFit::Crop,
            crop_left: self.left,
            crop_top: self.top,
            zoom_milli: (self.zoom.clamp(1.0, 4.0) * 1_000.0).round() as u16,
        })
    }

    fn stop_playback(&mut self) {
        self.is_playing = false;
        self.playback_restart_pending = false;
        if let Some(control) = &self.playback_control {
            control.stop.store(true, Ordering::Relaxed);
            let pid = control.pid.load(Ordering::Relaxed);
            if pid > 0 { terminate_preview_process(pid); }
        }
        self.playback_texture = None;
    }

    fn pause_playback(&mut self) {
        self.is_playing = false;
        self.playback_restart_pending = false;
        if let Some(control) = &self.playback_control {
            control.stop.store(true, Ordering::Relaxed);
            let pid = control.pid.load(Ordering::Relaxed);
            if pid > 0 { terminate_preview_process(pid); }
        }
    }

    fn start_playback(&mut self) {
        if self.playback_worker.is_some() { return; }
        let (Some(path), Some(info)) = (self.path.clone(), self.info) else { return; };
        let sticker_mode = self.show_sticker_preview;
        let (width, height, fps, start_ms, duration_ms, filter) = if sticker_mode {
            let start_ms = if self.playhead_ms < self.start_ms || self.playhead_ms >= self.end_ms {
                self.start_ms
            } else {
                self.playhead_ms
            };
            let duration_ms = self.end_ms.saturating_sub(start_ms);
            if duration_ms == 0 { return; }
            let filter = sticker_preview_filter(info, self.left, self.top, self.zoom);
            (512, 512, STICKER_PREVIEW_FPS, start_ms, duration_ms, filter)
        } else {
            if self.playhead_ms >= info.duration_ms.saturating_sub(1) { self.playhead_ms = 0; }
            let duration_ms = info.duration_ms.saturating_sub(self.playhead_ms);
            if duration_ms == 0 { return; }
            let (width, height) = playback_dimensions(info);
            let filter = source_playback_filter(width, height, PLAYBACK_FPS);
            (width, height, PLAYBACK_FPS, self.playhead_ms, duration_ms, filter)
        };
        if self.playback_is_sticker != sticker_mode {
            self.playback_texture = None;
        }
        self.playback_is_sticker = sticker_mode;
        self.playback_restart_pending = false;
        self.playhead_ms = start_ms;
        self.playback_error = None;
        self.playback_generation = self.playback_generation.wrapping_add(1);
        let generation = self.playback_generation;
        let stop = Arc::new(AtomicBool::new(false));
        let pid = Arc::new(AtomicI32::new(0));
        self.playback_control = Some(PlaybackControl { stop: stop.clone(), pid: pid.clone() });
        let tx = self.playback_tx.clone();
        self.is_playing = true;
        self.playback_worker = Some(thread::spawn(move || {
            stream_preview(path, width, height, fps, start_ms, duration_ms, filter, generation, stop, pid, tx);
        }));
    }

    pub(super) fn render(&mut self, ui: &mut egui::Ui) {
        // Keep the editor at a comfortable reading width and center it in the
        // workspace instead of letting the preview hug the left edge.
        let available_width = ui.available_width();
        let editor_width = available_width.min(1_080.0);
        let side_padding = ((available_width - editor_width) * 0.5).max(0.0);

        ui.horizontal(|ui| {
            if side_padding > 0.0 {
                ui.add_space(side_padding);
            }
            ui.vertical(|ui| {
                ui.set_width(editor_width);
                self.render_editor_contents(ui);
            });
        });
    }

    fn render_editor_contents(&mut self, ui: &mut egui::Ui) {
        if let Some(error) = &self.error {
            ui.colored_label(RED, error);
            return;
        }
        let Some(info) = self.info else {
            ui.vertical_centered(|ui| {
                ui.add_space(18.0);
                ui.spinner();
                ui.label("Analyzing video with FFprobe…");
                ui.add_space(18.0);
            });
            return;
        };

        // Size the preview from the actual window, not the remaining height of a
        // nested ScrollArea child UI. In that context available_height can be tiny.
        let preview_max_height = (ui.ctx().screen_rect().height() * 0.52).clamp(420.0, 520.0);
        ui.vertical(|ui| {
            egui::Frame::none()
                .fill(PREVIEW)
                .stroke(Stroke::new(1.0_f32, BORDER))
                .rounding(8.0)
                .inner_margin(egui::Margin::symmetric(8.0, 8.0))
                .show(ui, |preview_ui| {
                    preview_ui.set_width(preview_ui.available_width());
                    self.draw_preview(preview_ui, info, preview_max_height);
                });

            ui.add_space(8.0);
            let button_width = ((ui.available_width() - 8.0) * 0.5).max(1.0);
            ui.horizontal(|ui| {
                let source_active = self.is_playing && !self.show_sticker_preview;
                let sticker_active = (self.is_playing || self.playback_restart_pending) && self.show_sticker_preview;
                let worker_busy = self.playback_worker.is_some();
                let source_label = if source_active { "Ⅱ  Pause source" } else if worker_busy && !source_active { "Stopping…" } else { "▶  Play source" };
                if ui.add_enabled(
                    source_active || (!worker_busy && !self.is_playing),
                    egui::Button::new(RichText::new(source_label).strong())
                        .min_size(Vec2::new(button_width, 38.0))
                        .fill(if source_active { ACTIVE_BG } else { CARD }),
                ).clicked() {
                    if source_active {
                        self.pause_playback();
                    } else {
                        self.show_sticker_preview = false;
                        self.start_playback();
                    }
                }

                let sticker_label = if sticker_active { "Ⅱ  Pause sticker" } else if worker_busy && !sticker_active { "Stopping…" } else { "▶  Preview sticker" };
                if ui.add_enabled(
                    sticker_active || (!worker_busy && !self.is_playing),
                    egui::Button::new(RichText::new(sticker_label).strong())
                        .min_size(Vec2::new(button_width, 38.0))
                        .fill(if sticker_active { ACTIVE_BG } else { ACCENT }),
                ).clicked() {
                    if sticker_active {
                        self.pause_playback();
                    } else {
                        self.show_sticker_preview = true;
                        self.start_playback();
                    }
                }
            });
            ui.add_space(4.0);
            ui.label(RichText::new("Space: play / pause").size(10.5).color(TEXT_MUTED));
            if let Some(error) = &self.playback_error {
                ui.colored_label(RED, error);
            }

            ui.add_space(8.0);
            egui::Frame::none()
                .fill(PANEL)
                .stroke(Stroke::new(1.0_f32, BORDER))
                .rounding(8.0)
                .inner_margin(egui::Margin::symmetric(11.0, 10.0))
                .show(ui, |timeline_ui| {
                    timeline_ui.set_width(timeline_ui.available_width());
                    timeline_ui.horizontal(|timeline_ui| {
                        timeline_ui.label(RichText::new("TRIM CLIP").strong().size(11.0).color(CYAN));
                        timeline_ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |timeline_ui| {
                            timeline_ui.label(RichText::new(format!("{:.2}s / 3.00s max", (self.end_ms - self.start_ms) as f32 / 1_000.0))
                                .monospace().size(10.5).color(TEXT_SECONDARY));
                        });
                    });
                    timeline_ui.add_space(5.0);
                    self.draw_timeline(timeline_ui, info);
                    timeline_ui.add_space(5.0);
                    timeline_ui.horizontal(|timeline_ui| {
                        timeline_ui.label(RichText::new(format!("IN  {}", time_label(self.start_ms))).monospace().size(10.5).color(CYAN));
                        timeline_ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |timeline_ui| {
                            timeline_ui.label(RichText::new(format!("OUT  {}", time_label(self.end_ms))).monospace().size(10.5).color(CYAN));
                        });
                    });
                    timeline_ui.label(RichText::new(format!("PLAYHEAD  {}", time_label(self.playhead_ms))).monospace().size(10.0).color(TEXT_MUTED));
                });
        });

        if !ui.ctx().wants_keyboard_input()
            && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Space))
        {
            if self.is_playing {
                self.pause_playback();
            } else if self.playback_worker.is_none() {
                self.start_playback();
            }
        }
    }

    fn draw_preview(&mut self, ui: &mut egui::Ui, info: VideoInfo, max_height: f32) {
        if self.show_sticker_preview {
            self.draw_sticker_preview(ui, info, max_height);
            return;
        }
        let texture_data = if !self.playback_is_sticker {
            self.playback_texture.as_ref().or(self.texture.as_ref())
        } else {
            self.texture.as_ref()
        }.map(|texture| (texture.id(), texture.size_vec2()));

        let Some((texture_id, source_size)) = texture_data else {
            let (preview_width, preview_height) = playback_dimensions(info);
            let max_width = ui.available_width().min(1_000.0);
            let scale = (max_width / preview_width as f32).min(max_height / preview_height as f32);
            let display_size = Vec2::new(preview_width as f32, preview_height as f32) * scale;
            let row_width = ui.available_width();
            ui.horizontal(|ui| {
                ui.add_space(((row_width - display_size.x) * 0.5).max(0.0));
                let (rect, _) = ui.allocate_exact_size(display_size, Sense::hover());
                ui.painter().rect_filled(rect, 7.0, PREVIEW);
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER,
                    if self.is_playing { "Starting playback…" } else { "Loading source frame…" },
                    FontId::proportional(13.0), TEXT_MUTED);
            });
            if !self.is_playing {
                if let Some(error) = &self.frame_error {
                    ui.vertical_centered(|ui| { ui.colored_label(RED, error); });
                }
            }
            return;
        };

        let max_width = ui.available_width().min(1_000.0);
        let scale = (max_width / source_size.x).min(max_height / source_size.y);
        let display_size = source_size * scale;
        let row_width = ui.available_width();
        ui.horizontal(|ui| {
            ui.add_space(((row_width - display_size.x) * 0.5).max(0.0));
            let (image_rect, _) = ui.allocate_exact_size(display_size, Sense::hover());
            ui.painter().rect_filled(image_rect, 5.0, PREVIEW);
            ui.painter().image(texture_id, image_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);

            let crop = crop_rect_for_preview(info, image_rect, self.left, self.top, self.zoom);
            let shade = Color32::from_black_alpha(145);
            for r in [
                egui::Rect::from_min_max(image_rect.min, egui::pos2(image_rect.right(), crop.top())),
                egui::Rect::from_min_max(egui::pos2(image_rect.left(), crop.bottom()), image_rect.max),
                egui::Rect::from_min_max(egui::pos2(image_rect.left(), crop.top()), egui::pos2(crop.left(), crop.bottom())),
                egui::Rect::from_min_max(egui::pos2(crop.right(), crop.top()), egui::pos2(image_rect.right(), crop.bottom())),
            ] {
                if r.is_positive() { ui.painter().rect_filled(r, 0.0, shade); }
            }
            ui.painter().rect_stroke(crop, 2.0, Stroke::new(2.0_f32, CYAN));
            self.draw_crop_handles(ui, image_rect, crop, info);
        });
        if !self.is_playing {
            if let Some(error) = &self.frame_error {
                ui.vertical_centered(|ui| { ui.colored_label(RED, error); });
            }
        }
    }

    fn draw_sticker_preview(&mut self, ui: &mut egui::Ui, info: VideoInfo, max_height: f32) {
        let texture_data = if self.playback_is_sticker {
            self.playback_texture.as_ref().or(self.texture.as_ref())
        } else {
            self.texture.as_ref()
        }.map(|texture| (texture.id(), texture.size_vec2()));
        let side = ui.available_width().min(max_height).min(500.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());
        ui.painter().rect_filled(rect, 8.0, Color32::BLACK);
        let Some((texture_id, source_size)) = texture_data else {
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER,
                "Preparing sticker preview…", FontId::proportional(13.0), TEXT_MUTED);
            return;
        };

        if self.playback_is_sticker && self.playback_texture.is_some() {
            ui.painter().image(texture_id, rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
        } else {
            let source_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, source_size);
            let crop = crop_rect_for_preview(info, source_rect, self.left, self.top, self.zoom);
            let uv = egui::Rect::from_min_max(
                egui::pos2(crop.left() / source_size.x, crop.top() / source_size.y),
                egui::pos2(crop.right() / source_size.x, crop.bottom() / source_size.y),
            );
            ui.painter().image(texture_id, rect, uv, Color32::WHITE);
        }
        ui.painter().rect_stroke(rect, 8.0, Stroke::new(1.0_f32, BORDER));
        ui.painter().text(egui::pos2(rect.center().x, rect.bottom() - 9.0), egui::Align2::CENTER_BOTTOM,
            format!("Sticker · {}", time_label(self.playhead_ms)), FontId::proportional(10.5), Color32::from_white_alpha(190));
    }

    fn draw_crop_handles(&mut self, ui: &mut egui::Ui, image_rect: egui::Rect, crop: egui::Rect, info: VideoInfo) {
        let corners = [crop.left_top(), crop.right_top(), crop.left_bottom(), crop.right_bottom()];
        let center = ui.interact(
            crop.shrink(CROP_HANDLE_SIZE * 0.75),
            ui.id().with("sticker-crop-move"),
            Sense::drag(),
        );
        if center.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::Grab); }
        if center.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            let delta = ui.input(|input| input.pointer.delta());
            let side = info.width.min(info.height) as f32 / self.zoom.clamp(1.0, 4.0);
            let available_x = info.width as f32 - side;
            let available_y = info.height as f32 - side;
            let scale = image_rect.width() / info.width as f32;
            if available_x > 0.0 {
                self.left = (self.left as f32 + delta.x / (available_x * scale) * 1_000.0).round().clamp(0.0, 1_000.0) as u16;
            }
            if available_y > 0.0 {
                self.top = (self.top as f32 + delta.y / (available_y * scale) * 1_000.0).round().clamp(0.0, 1_000.0) as u16;
            }
        }

        for (index, point) in corners.iter().copied().enumerate() {
            let handle_rect = egui::Rect::from_center_size(point, Vec2::splat(CROP_HANDLE_SIZE));
            let response = ui.interact(handle_rect, ui.id().with(("sticker-crop-corner", index)), Sense::drag());
            ui.painter().rect_filled(
                egui::Rect::from_center_size(point, Vec2::splat(10.0)),
                2.0,
                CYAN,
            );
            ui.painter().rect_stroke(
                egui::Rect::from_center_size(point, Vec2::splat(10.0)),
                2.0,
                Stroke::new(1.0_f32, Color32::WHITE),
            );
            if response.hovered() || response.dragged() {
                ui.ctx().set_cursor_icon(if index == 0 || index == 3 { egui::CursorIcon::ResizeNwSe } else { egui::CursorIcon::ResizeNeSw });
            }
            if response.dragged() {
                if let Some(pointer) = ui.input(|input| input.pointer.interact_pos()) {
                    self.resize_crop_from_corner(info, image_rect, index, pointer);
                }
            }
        }
    }

    fn resize_crop_from_corner(&mut self, info: VideoInfo, image_rect: egui::Rect, corner: usize, pointer: egui::Pos2) {
        let minimum_side = (info.width.min(info.height) & !1).max(2) as f32;
        let current_side = minimum_side / self.zoom.clamp(1.0, 4.0);
        let max_left = (info.width as f32 - current_side).max(0.0);
        let max_top = (info.height as f32 - current_side).max(0.0);
        let current_left = max_left * self.left as f32 / 1_000.0;
        let current_top = max_top * self.top as f32 / 1_000.0;

        let drag_left = corner == 0 || corner == 2;
        let drag_top = corner == 0 || corner == 1;
        let anchor_x = if drag_left { current_left + current_side } else { current_left };
        let anchor_y = if drag_top { current_top + current_side } else { current_top };
        let screen_scale = image_rect.width() / info.width as f32;
        let screen_anchor = egui::pos2(
            image_rect.left() + anchor_x * screen_scale,
            image_rect.top() + anchor_y * screen_scale,
        );
        let dx = (pointer.x - screen_anchor.x).abs();
        let dy = (pointer.y - screen_anchor.y).abs();
        let wanted_screen_side = (dx + dy) * 0.5;
        let wanted_source_side = (wanted_screen_side / screen_scale)
            .clamp(minimum_side / 4.0, minimum_side)
            .round()
            .max(2.0);
        let new_zoom = (minimum_side / wanted_source_side).clamp(1.0, 4.0);
        let new_side = minimum_side / new_zoom;
        let new_left = (if drag_left { anchor_x - new_side } else { anchor_x })
            .clamp(0.0, (info.width as f32 - new_side).max(0.0));
        let new_top = (if drag_top { anchor_y - new_side } else { anchor_y })
            .clamp(0.0, (info.height as f32 - new_side).max(0.0));
        let new_max_left = (info.width as f32 - new_side).max(0.0);
        let new_max_top = (info.height as f32 - new_side).max(0.0);
        self.zoom = new_zoom;
        self.left = if new_max_left > 0.0 { (new_left / new_max_left * 1_000.0).round().clamp(0.0, 1_000.0) as u16 } else { 0 };
        self.top = if new_max_top > 0.0 { (new_top / new_max_top * 1_000.0).round().clamp(0.0, 1_000.0) as u16 } else { 0 };
    }

    fn draw_timeline(&mut self, ui: &mut egui::Ui, info: VideoInfo) {
        egui::Frame::none()
            .fill(INPUT)
            .stroke(Stroke::new(1.0_f32, BORDER))
            .rounding(7.0)
            .inner_margin(egui::Margin::symmetric(10.0, 8.0))
            .show(ui, |ui| {
                let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 62.0), Sense::click_and_drag());
                let track_left = rect.left() + 8.0;
                let track_right = rect.right() - 8.0;
                let track_width = (track_right - track_left).max(1.0);
                let x_at = |ms: u32| track_left + track_width * ms as f32 / info.duration_ms.max(1) as f32;
                let ms_at = |x: f32| {
                    (((x - track_left) / track_width).clamp(0.0, 1.0) * info.duration_ms as f32).round() as u32
                };

                let ruler_y = rect.top() + 7.0;
                for tick in 0..=4 {
                    let x = track_left + track_width * tick as f32 / 4.0;
                    ui.painter().line_segment(
                        [egui::pos2(x, ruler_y + 2.0), egui::pos2(x, ruler_y + 9.0)],
                        Stroke::new(1.0_f32, BORDER_MUTED),
                    );
                    let time = info.duration_ms.saturating_mul(tick) / 4;
                    ui.painter().text(
                        egui::pos2(x, ruler_y - 1.0),
                        if tick == 0 { egui::Align2::LEFT_BOTTOM } else if tick == 4 { egui::Align2::RIGHT_BOTTOM } else { egui::Align2::CENTER_BOTTOM },
                        time_label(time),
                        FontId::proportional(9.5),
                        TEXT_MUTED,
                    );
                }

                let track = egui::Rect::from_min_max(
                    egui::pos2(track_left, rect.top() + 26.0),
                    egui::pos2(track_right, rect.top() + 43.0),
                );
                ui.painter().rect_filled(track, 4.0, CARD);
                let start_x = x_at(self.start_ms);
                let end_x = x_at(self.end_ms);
                let selected = egui::Rect::from_min_max(
                    egui::pos2(start_x, track.top()),
                    egui::pos2(end_x, track.bottom()),
                );
                ui.painter().rect_filled(selected, 3.0, BORDER);
                ui.painter().rect_stroke(selected, 3.0, Stroke::new(1.0_f32, CYAN_SOFT));

                for x in [start_x, end_x] {
                    ui.painter().line_segment(
                        [egui::pos2(x, track.top() - 5.0), egui::pos2(x, track.bottom() + 5.0)],
                        Stroke::new(2.0_f32, CYAN_SOFT),
                    );
                    ui.painter().circle_filled(egui::pos2(x, track.center().y), 5.0, CYAN_SOFT);
                    ui.painter().circle_stroke(egui::pos2(x, track.center().y), 5.0, Stroke::new(1.0_f32, Color32::WHITE));
                }

                let play_x = x_at(self.playhead_ms);
                ui.painter().line_segment(
                    [egui::pos2(play_x, rect.top() + 12.0), egui::pos2(play_x, rect.bottom() - 3.0)],
                    Stroke::new(2.0_f32, WARNING),
                );
                ui.painter().circle_filled(egui::pos2(play_x, rect.top() + 13.0), 4.0, WARNING);

                if response.hovered() {
                    if let Some(pointer) = ui.input(|input| input.pointer.hover_pos()) {
                        let near_playhead = (pointer.x - play_x).abs() <= 8.0;
                        let near_edge = (pointer.x - start_x).abs() <= 14.0 || (pointer.x - end_x).abs() <= 14.0;
                        if pointer.y >= track.top() && pointer.y <= track.bottom()
                            && pointer.x > start_x && pointer.x < end_x
                            && !near_playhead && !near_edge
                        {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                        }
                    }
                }

                if response.drag_started() {
                    if let Some(pointer) = ui.input(|input| input.pointer.interact_pos()) {
                        let value = ms_at(pointer.x);
                        let start_distance = (pointer.x - start_x).abs();
                        let end_distance = (pointer.x - end_x).abs();
                        let playhead_distance = (pointer.x - play_x).abs();
                        // Trim handles take priority over the playhead when they overlap,
                        // which happens at the start of a newly loaded clip (time = 0).
                        self.timeline_drag = Some(if start_distance <= 14.0 && start_distance <= end_distance {
                            TimelineDrag::Start
                        } else if end_distance <= 14.0 {
                            TimelineDrag::End
                        } else if playhead_distance <= 8.0 {
                            TimelineDrag::Playhead
                        } else if pointer.x > start_x && pointer.x < end_x {
                            TimelineDrag::Selection { pointer_anchor_ms: value, start_ms: self.start_ms, end_ms: self.end_ms }
                        } else {
                            TimelineDrag::Playhead
                        });
                        if matches!(self.timeline_drag, Some(TimelineDrag::Selection { .. })) {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                        }
                    }
                }

                if response.dragged() {
                    if let Some(pointer) = ui.input(|input| input.pointer.interact_pos()) {
                        let value = ms_at(pointer.x);
                        match self.timeline_drag.unwrap_or(TimelineDrag::Playhead) {
                            TimelineDrag::Start => {
                                let earliest = self.end_ms.saturating_sub(3_000);
                                let latest = self.end_ms.saturating_sub(100);
                                self.start_ms = value.clamp(earliest, latest);
                            }
                            TimelineDrag::End => {
                                let earliest = self.start_ms.saturating_add(100);
                                let latest = self.start_ms.saturating_add(3_000).min(info.duration_ms);
                                self.end_ms = value.clamp(earliest, latest.max(earliest));
                            }
                            TimelineDrag::Playhead => {
                                let next = value.min(info.duration_ms.saturating_sub(1));
                                if next != self.playhead_ms {
                                    self.playhead_ms = next;
                                    self.stop_playback();
                                }
                            }
                            TimelineDrag::Selection { pointer_anchor_ms, start_ms, end_ms } => {
                                (self.start_ms, self.end_ms) = move_selection(start_ms, end_ms, pointer_anchor_ms, value, info.duration_ms);
                                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                            }
                        }
                    }
                } else if response.clicked() {
                    if let Some(pointer) = ui.input(|input| input.pointer.interact_pos()) {
                        self.playhead_ms = ms_at(pointer.x).min(info.duration_ms.saturating_sub(1));
                        self.stop_playback();
                    }
                }
                if response.drag_stopped() { self.timeline_drag = None; }
            });
    }
}

impl Drop for VideoStickerEditor {
    fn drop(&mut self) {
        self.stop_playback();
        if let Some(worker) = self.playback_worker.take() { let _ = worker.join(); }
        for worker in &self.workers {
            worker.cancel();
        }
        for worker in self.workers.drain(..) {
            worker.join();
        }
    }
}

fn crop_rect_for_preview(info: VideoInfo, image_rect: egui::Rect, left: u16, top: u16, zoom: f32) -> egui::Rect {
    let side = info.width.min(info.height) as f32 / zoom.clamp(1.0, 4.0);
    let crop_width = image_rect.width() * side / info.width as f32;
    let crop_height = image_rect.height() * side / info.height as f32;
    let x = image_rect.left() + (image_rect.width() - crop_width) * left as f32 / 1_000.0;
    let y = image_rect.top() + (image_rect.height() - crop_height) * top as f32 / 1_000.0;
    egui::Rect::from_min_size(egui::pos2(x, y), Vec2::new(crop_width, crop_height))
}

fn playback_dimensions(info: VideoInfo) -> (u32, u32) {
    let longest = info.width.max(info.height).max(2) as f32;
    let scale = PLAYBACK_MAX_SIDE as f32 / longest;
    let width = ((info.width as f32 * scale).round() as u32).max(2) & !1;
    let height = ((info.height as f32 * scale).round() as u32).max(2) & !1;
    (width, height)
}

fn source_playback_filter(width: u32, height: u32, fps: u32) -> String {
    format!("scale={width}:{height}:flags=fast_bilinear,setsar=1,fps={fps},format=rgba")
}

fn sticker_preview_filter(info: VideoInfo, left: u16, top: u16, zoom: f32) -> String {
    let minimum_side = (info.width.min(info.height) & !1).max(2);
    let crop_side = ((minimum_side as f32 / zoom.clamp(1.0, 4.0)) as u32 & !1).max(2);
    let x = ((info.width.saturating_sub(crop_side) as u64 * u64::from(left.min(1_000))) / 1_000) as u32 & !1;
    let y = ((info.height.saturating_sub(crop_side) as u64 * u64::from(top.min(1_000))) / 1_000) as u32 & !1;
    format!("crop={crop_side}:{crop_side}:{x}:{y},scale=512:512:flags=fast_bilinear,setsar=1,fps={STICKER_PREVIEW_FPS},format=rgba")
}

fn move_selection(
    start_ms: u32,
    end_ms: u32,
    pointer_anchor_ms: u32,
    pointer_ms: u32,
    duration_ms: u32,
) -> (u32, u32) {
    let selection_length = end_ms.saturating_sub(start_ms);
    if selection_length == 0 || selection_length > duration_ms {
        return (start_ms.min(duration_ms), end_ms.min(duration_ms));
    }
    let delta = i64::from(pointer_ms) - i64::from(pointer_anchor_ms);
    let max_start = duration_ms.saturating_sub(selection_length);
    let next_start = (i64::from(start_ms) + delta).clamp(0, i64::from(max_start)) as u32;
    (next_start, next_start + selection_length)
}

fn stream_preview(
    path: PathBuf,
    width: u32,
    height: u32,
    fps: u32,
    start_ms: u32,
    duration_ms: u32,
    filter: String,
    generation: u64,
    stop: Arc<AtomicBool>,
    pid: Arc<AtomicI32>,
    tx: SyncSender<PlaybackMessage>,
) {
    let seek = format!("{:.3}", start_ms as f64 / 1_000.0);
    let duration = format!("{:.3}", duration_ms as f64 / 1_000.0);
    let mut command = Command::new("ffmpeg");
    command.args([
        "-hide_banner", "-loglevel", "error", "-nostdin", "-re",
        "-ss", &seek, "-i",
    ])
    .arg(&path)
    .args([
        "-t", &duration, "-map", "0:v:0", "-vf", &filter,
        "-an", "-sn", "-dn", "-threads", "2",
        "-pix_fmt", "rgba", "-f", "rawvideo", "pipe:1",
    ])
    .stdin(Stdio::null())
    .stderr(Stdio::null())
    .stdout(Stdio::piped());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            let _ = tx.try_send(PlaybackMessage::Error {
                generation,
                message: format!("Could not start FFmpeg playback: {error}"),
            });
            return;
        }
    };
    pid.store(child.id() as i32, Ordering::Relaxed);
    let mut stdout = child.stdout.take().expect("FFmpeg preview stdout is piped");
    let frame_bytes = (width as usize).saturating_mul(height as usize).saturating_mul(4);
    let mut frame_index = 0_u64;
    let mut read_error = None;

    loop {
        if stop.load(Ordering::Relaxed) {
            stop_preview_child(&mut child);
            break;
        }
        let mut pixels = vec![0_u8; frame_bytes];
        match stdout.read_exact(&mut pixels) {
            Ok(()) => {
                if stop.load(Ordering::Relaxed) {
                    stop_preview_child(&mut child);
                    break;
                }
                let offset_ms = frame_index.saturating_mul(1_000) / u64::from(fps.max(1));
                let time_ms = start_ms.saturating_add(offset_ms.min(u64::from(u32::MAX)) as u32).min(start_ms.saturating_add(duration_ms));
                let _ = tx.try_send(PlaybackMessage::Frame { generation, time_ms, pixels });
                frame_index += 1;
            }
            Err(error) if error.kind() == ErrorKind::UnexpectedEof => break,
            Err(error) => {
                read_error = Some(error.to_string());
                break;
            }
        }
    }

    let status = child.wait();
    pid.store(0, Ordering::Relaxed);
    if stop.load(Ordering::Relaxed) { return; }
    if let Some(error) = read_error {
        let _ = tx.try_send(PlaybackMessage::Error { generation, message: format!("Could not read FFmpeg preview frames: {error}") });
    } else {
        match status {
            Ok(status) if status.success() => {
                let _ = tx.try_send(PlaybackMessage::Finished { generation });
            }
            Ok(status) => {
                let _ = tx.try_send(PlaybackMessage::Error {
                    generation,
                    message: format!("FFmpeg preview stopped with exit code {}.", status),
                });
            }
            Err(error) => {
                let _ = tx.try_send(PlaybackMessage::Error { generation, message: format!("Could not finish FFmpeg preview: {error}") });
            }
        }
    }
}

fn stop_preview_child(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        let group = -(child.id() as libc::pid_t);
        let _ = unsafe { libc::kill(group, libc::SIGKILL) };
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
    }
    let _ = child.wait();
}

fn terminate_preview_process(pid: i32) {
    #[cfg(unix)]
    {
        let _ = unsafe { libc::kill(-(pid as libc::pid_t), libc::SIGKILL) };
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
    }
}

fn run_preview_command(mut command: Command, cancelled: &AtomicBool) -> Result<std::process::Output, String> {
    if cancelled.load(Ordering::Acquire) {
        return Err("Preview cancelled.".into());
    }
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn().map_err(|error| error.to_string())?;
    let stdout = child.stdout.take().expect("preview stdout is piped");
    let stderr = child.stderr.take().expect("preview stderr is piped");
    let stdout_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut pipe = stdout;
        pipe.read_to_end(&mut bytes).map(|_| bytes)
    });
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut pipe = stderr;
        pipe.read_to_end(&mut bytes).map(|_| bytes)
    });

    let status = loop {
        if cancelled.load(Ordering::Acquire) {
            stop_preview_child(&mut child);
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err("Preview cancelled.".into());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(error) => {
                stop_preview_child(&mut child);
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(error.to_string());
            }
        }
    };

    let stdout = stdout_reader.join()
        .map_err(|_| "FFmpeg preview output reader panicked.".to_string())?
        .map_err(|error| format!("Could not read preview output: {error}"))?;
    let stderr = stderr_reader.join()
        .map_err(|_| "FFmpeg preview error reader panicked.".to_string())?
        .map_err(|error| format!("Could not read preview errors: {error}"))?;
    Ok(std::process::Output { status, stdout, stderr })
}

fn display_oriented_dimensions(width: u32, height: u32, rotation: Option<f64>) -> (u32, u32) {
    let rotation = rotation.unwrap_or(0.0).round() as i32;
    if rotation.rem_euclid(180) == 90 { (height, width) } else { (width, height) }
}

fn rational_value(value: &str) -> Option<f64> {
    let (numerator, denominator) = value.split_once('/')?;
    let numerator = numerator.trim().parse::<f64>().ok()?;
    let denominator = denominator.trim().parse::<f64>().ok()?;
    if !numerator.is_finite() || !denominator.is_finite() || denominator == 0.0 {
        return None;
    }
    Some(numerator / denominator)
}

fn numeric_value(value: &Value) -> Option<f64> {
    value.as_f64()
        .or_else(|| value.as_str().and_then(|text| text.parse::<f64>().ok()))
        .filter(|number| number.is_finite())
}

fn stream_duration_seconds(stream: &Value) -> Option<f64> {
    stream.get("duration")
        .and_then(numeric_value)
        .filter(|value| *value > 0.0)
        .or_else(|| {
            let ticks = numeric_value(stream.get("duration_ts")?)?;
            let time_base = stream.get("time_base").and_then(Value::as_str)?;
            let seconds_per_tick = rational_value(time_base)?;
            let duration = ticks * seconds_per_tick;
            (duration.is_finite() && duration > 0.0).then_some(duration)
        })
        .or_else(|| {
            let frames = numeric_value(stream.get("nb_frames")?)?;
            let frame_rate = stream.get("avg_frame_rate").and_then(Value::as_str)?;
            let frames_per_second = rational_value(frame_rate)?;
            if frames_per_second <= 0.0 { return None; }
            let duration = frames / frames_per_second;
            (duration.is_finite() && duration > 0.0).then_some(duration)
        })
}

fn inspect_video(path: &Path, cancelled: &AtomicBool) -> Result<VideoInfo, String> {
    let mut command = Command::new("ffprobe");
    command.args([
        "-v", "error",
        "-select_streams", "v:0",
        // Use the long-standing show_streams/show_format flags instead of
        // naming stream_side_data in -show_entries. Older FFprobe releases
        // (including Ubuntu 22.04's) reject that section name, even though
        // they can return side_data_list as part of each stream object.
        "-show_streams",
        "-show_format",
        "-of", "json",
    ]).arg(path);
    let output = run_preview_command(command, cancelled)
        .map_err(|e| format!("Could not inspect the selected video: {e}. Install FFmpeg and FFprobe if they are missing."))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(if detail.is_empty() { "FFprobe could not read the selected video.".into() } else { detail });
    }
    let value: Value = serde_json::from_slice(&output.stdout).map_err(|e| format!("FFprobe returned invalid metadata: {e}"))?;
    let stream = value["streams"].as_array().and_then(|s| s.first()).ok_or("The selected file does not contain a video stream.")?;
    let duration = value["format"]["duration"].as_str()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
        .or_else(|| stream_duration_seconds(stream))
        .unwrap_or(0.0);
    let width = stream["width"].as_u64().unwrap_or(0) as u32;
    let height = stream["height"].as_u64().unwrap_or(0) as u32;
    let rotation = stream["side_data_list"].as_array()
        .and_then(|items| items.iter().find_map(|item| item["rotation"].as_f64()))
        .or_else(|| stream["tags"]["rotate"].as_str().and_then(|value| value.parse::<f64>().ok()));
    let (width, height) = display_oriented_dimensions(width, height, rotation);
    if width < 2 || height < 2 || !duration.is_finite() || duration <= 0.0 {
        return Err("FFprobe could not determine the video dimensions or duration.".into());
    }
    Ok(VideoInfo { width, height, duration_ms: (duration * 1_000.0).round().clamp(1.0, u32::MAX as f64) as u32 })
}

fn preview_frame(path: &Path, time_ms: u32, cancelled: &AtomicBool) -> Result<egui::ColorImage, String> {
    let seek = format!("{:.3}", time_ms as f64 / 1_000.0);
    let mut command = Command::new("ffmpeg");
    command.args(["-hide_banner", "-loglevel", "error", "-nostdin", "-ss", &seek, "-i"])
        .arg(path)
        .args(["-frames:v", "1", "-vf", "scale=512:512:force_original_aspect_ratio=decrease:flags=fast_bilinear", "-f", "image2pipe", "-vcodec", "png", "pipe:1"]);
    let output = run_preview_command(command, cancelled)
        .map_err(|e| format!("Could not render FFmpeg preview: {e}"))?;
    if !output.status.success() || output.stdout.is_empty() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(if detail.is_empty() { "FFmpeg could not render a preview frame.".into() } else { detail });
    }
    let image = image::load_from_memory(&output.stdout).map_err(|e| format!("Could not decode the preview frame: {e}"))?.to_rgba8();
    Ok(egui::ColorImage::from_rgba_unmultiplied([image.width() as usize, image.height() as usize], image.as_raw()))
}

fn time_label(ms: u32) -> String {
    let seconds = ms / 1_000;
    format!("{:02}:{:02}.{:02}", seconds / 60, seconds % 60, (ms % 1_000) / 10)
}

#[cfg(test)]
mod tests {
    use super::{crop_rect_for_preview, sticker_preview_filter, VideoInfo};
    use eframe::egui;

    #[test]
    fn square_crop_preview_is_positioned_and_zoomed_within_source_bounds() {
        let info = VideoInfo { width: 1920, height: 1080, duration_ms: 10_000 };
        let image_rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(640.0, 360.0));
        let centered = crop_rect_for_preview(info, image_rect, 500, 500, 1.0);
        assert!((centered.width() - 360.0).abs() < 0.01);
        assert!((centered.height() - 360.0).abs() < 0.01);
        let zoomed = crop_rect_for_preview(info, image_rect, 1_000, 1_000, 2.0);
        assert!((zoomed.width() - 180.0).abs() < 0.01);
        assert!((zoomed.height() - 180.0).abs() < 0.01);
        assert!(zoomed.right() <= image_rect.right() + 0.01);
        assert!(zoomed.bottom() <= image_rect.bottom() + 0.01);
    }

    #[test]
    fn playback_dimensions_preserve_landscape_portrait_and_square_aspect_ratios() {
        assert_eq!(super::playback_dimensions(VideoInfo { width: 1_920, height: 1_080, duration_ms: 10_000 }), (512, 288));
        assert_eq!(super::playback_dimensions(VideoInfo { width: 1_080, height: 1_920, duration_ms: 10_000 }), (288, 512));
        assert_eq!(super::playback_dimensions(VideoInfo { width: 600, height: 600, duration_ms: 10_000 }), (512, 512));
    }

    #[test]
    fn source_playback_filter_scales_the_original_frame_without_sticker_cropping() {
        let filter = super::source_playback_filter(512, 288, 24);
        assert_eq!(filter, "scale=512:288:flags=fast_bilinear,setsar=1,fps=24,format=rgba");
        assert!(!filter.contains("crop="));
    }

    #[test]
    fn sticker_preview_filter_matches_the_export_crop_geometry_and_output() {
        let info = VideoInfo { width: 1_920, height: 1_080, duration_ms: 10_000 };
        assert_eq!(
            sticker_preview_filter(info, 500, 500, 2.0),
            "crop=540:540:690:270,scale=512:512:flags=fast_bilinear,setsar=1,fps=30,format=rgba"
        );
    }

    #[test]
    fn moving_timeline_selection_preserves_its_length_and_clamps_to_source() {
        assert_eq!(super::move_selection(1_000, 2_500, 1_500, 2_000, 10_000), (1_500, 3_000));
        assert_eq!(super::move_selection(1_000, 2_500, 1_500, 0, 10_000), (0, 1_500));
        assert_eq!(super::move_selection(8_000, 9_500, 8_500, 10_000, 10_000), (8_500, 10_000));
    }
}

#[cfg(all(test, unix))]
mod preview_cancellation_tests {
    use super::run_preview_command;
    use std::process::Command;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, Instant};

    #[test]
    fn cancelling_a_preview_terminates_its_process_group_promptly() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = cancelled.clone();
        let worker = thread::spawn(move || {
            let mut command = Command::new("sh");
            command.args(["-c", "sleep 10"]);
            run_preview_command(command, &worker_cancelled)
        });

        thread::sleep(Duration::from_millis(150));
        let started = Instant::now();
        cancelled.store(true, Ordering::Release);
        let result = worker.join().expect("preview worker should not panic");

        assert!(result.unwrap_err().contains("cancelled"));
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "preview cancellation should terminate the child process group promptly"
        );
    }
}


#[cfg(test)]
mod display_orientation_tests {
    use super::display_oriented_dimensions;

    #[test]
    fn display_matrix_rotation_swaps_portrait_dimensions() {
        assert_eq!(display_oriented_dimensions(1920, 1080, Some(90.0)), (1080, 1920));
        assert_eq!(display_oriented_dimensions(1920, 1080, Some(-90.0)), (1080, 1920));
        assert_eq!(display_oriented_dimensions(1920, 1080, Some(270.0)), (1080, 1920));
        assert_eq!(display_oriented_dimensions(1920, 1080, Some(180.0)), (1920, 1080));
        assert_eq!(display_oriented_dimensions(1920, 1080, None), (1920, 1080));
    }
}


#[cfg(test)]
mod duration_metadata_tests {
    use super::{rational_value, stream_duration_seconds};
    use serde_json::json;

    #[test]
    fn parses_ffprobe_rational_values() {
        assert_eq!(rational_value("1/1000"), Some(0.001));
        assert_eq!(rational_value("30000/1001"), Some(30000.0 / 1001.0));
        assert_eq!(rational_value("1/0"), None);
        assert_eq!(rational_value("N/A"), None);
    }

    #[test]
    fn calculates_duration_from_stream_timestamps_when_webm_format_duration_is_missing() {
        let stream = json!({
            "duration": "N/A",
            "duration_ts": 90000,
            "time_base": "1/90000"
        });
        assert_eq!(stream_duration_seconds(&stream), Some(1.0));
    }

    #[test]
    fn falls_back_to_frame_count_and_average_frame_rate() {
        let stream = json!({
            "duration": "N/A",
            "duration_ts": "N/A",
            "nb_frames": "60",
            "avg_frame_rate": "30/1"
        });
        assert_eq!(stream_duration_seconds(&stream), Some(2.0));
    }
}
