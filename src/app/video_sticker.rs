use crate::engine::{JobOptions, VideoStickerFit};
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

const PLAYBACK_WIDTH: u32 = 512;
const PLAYBACK_HEIGHT: u32 = 512;
const PLAYBACK_FPS: u32 = 24;
const CROP_HANDLE_SIZE: f32 = 22.0;

#[derive(Clone, Copy)]
struct VideoInfo { width: u32, height: u32, duration_ms: u32 }

enum Message {
    Metadata(PathBuf, Result<VideoInfo, String>),
    Frame(PathBuf, u32, Result<egui::ColorImage, String>),
}

enum PlaybackMessage {
    Frame { generation: u64, time_ms: u32, pixels: Vec<u8> },
    Finished { generation: u64 },
    Error { generation: u64, message: String },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TimelineDrag { Start, End, Playhead }

#[derive(Clone, Copy, PartialEq, Eq)]
enum CropDrag { Move, Resize(usize) }

struct PlaybackControl {
    stop: Arc<AtomicBool>,
    pid: Arc<AtomicI32>,
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
    fit: VideoStickerFit,
    left: u16,
    top: u16,
    zoom: f32,
    texture: Option<TextureHandle>,
    texture_time: Option<u32>,
    playback_texture: Option<TextureHandle>,
    metadata_pending: Option<PathBuf>,
    frame_pending: Option<(PathBuf, u32)>,
    timeline_drag: Option<TimelineDrag>,
    crop_drag: Option<CropDrag>,
    tx: Sender<Message>,
    rx: Receiver<Message>,
    workers: Vec<JoinHandle<()>>,
    playback_tx: SyncSender<PlaybackMessage>,
    playback_rx: Receiver<PlaybackMessage>,
    playback_control: Option<PlaybackControl>,
    playback_worker: Option<JoinHandle<()>>,
    playback_generation: u64,
    is_playing: bool,
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
            fit: VideoStickerFit::Crop,
            left: 500,
            top: 500,
            zoom: 1.0,
            texture: None,
            texture_time: None,
            playback_texture: None,
            metadata_pending: None,
            frame_pending: None,
            timeline_drag: None,
            crop_drag: None,
            tx,
            rx,
            workers: Vec::new(),
            playback_tx,
            playback_rx,
            playback_control: None,
            playback_worker: None,
            playback_generation: 0,
            is_playing: false,
        }
    }
}

impl VideoStickerEditor {
    pub(super) fn set_input(&mut self, path: Option<PathBuf>) {
        if self.path == path { return; }
        self.stop_playback();
        self.path = path.clone();
        self.info = None;
        self.error = None;
        self.frame_error = None;
        self.playback_error = None;
        self.start_ms = 0;
        self.end_ms = 0;
        self.playhead_ms = 0;
        self.fit = VideoStickerFit::Crop;
        self.left = 500;
        self.top = 500;
        self.zoom = 1.0;
        self.texture = None;
        self.texture_time = None;
        self.playback_texture = None;
        self.metadata_pending = None;
        self.frame_pending = None;
        self.timeline_drag = None;
        self.crop_drag = None;
        if let Some(path) = path {
            self.metadata_pending = Some(path.clone());
            let tx = self.tx.clone();
            self.workers.push(thread::spawn(move || {
                let result = inspect_video(&path);
                let _ = tx.send(Message::Metadata(path, result));
            }));
        }
    }

    pub(super) fn poll(&mut self, ctx: &egui::Context) {
        while let Ok(message) = self.rx.try_recv() {
            match message {
                Message::Metadata(path, result) => {
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
                Message::Frame(path, time_ms, result) => {
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
                    let image = egui::ColorImage::from_rgba_unmultiplied(
                        [PLAYBACK_WIDTH as usize, PLAYBACK_HEIGHT as usize],
                        &pixels,
                    );
                    self.playback_texture = Some(ctx.load_texture(
                        format!("video-sticker-playback-{generation}-{time_ms}"),
                        image,
                        egui::TextureOptions::LINEAR,
                    ));
                    self.playhead_ms = time_ms.min(self.end_ms);
                }
                PlaybackMessage::Finished { generation } if generation == self.playback_generation => {
                    self.is_playing = false;
                    self.playback_texture = None;
                    self.playhead_ms = self.end_ms;
                }
                PlaybackMessage::Error { generation, message } if generation == self.playback_generation => {
                    self.is_playing = false;
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
                self.playback_texture = None;
            }
        }

        let mut index = 0;
        while index < self.workers.len() {
            if self.workers[index].is_finished() {
                let worker = self.workers.swap_remove(index);
                let _ = worker.join();
            } else {
                index += 1;
            }
        }

        if !self.is_playing {
            if let (Some(path), Some(info)) = (self.path.clone(), self.info) {
                if self.frame_pending.is_none() && self.texture_time != Some(self.playhead_ms) {
                    let at = self.playhead_ms.min(info.duration_ms.saturating_sub(1));
                    self.frame_pending = Some((path.clone(), at));
                    let tx = self.tx.clone();
                    self.workers.push(thread::spawn(move || {
                        let result = preview_frame(&path, at);
                        let _ = tx.send(Message::Frame(path, at, result));
                    }));
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
            fit: self.fit,
            crop_left: self.left,
            crop_top: self.top,
            zoom_milli: (self.zoom.clamp(1.0, 4.0) * 1_000.0).round() as u16,
        })
    }

    fn stop_playback(&mut self) {
        self.is_playing = false;
        if let Some(control) = &self.playback_control {
            control.stop.store(true, Ordering::Relaxed);
            let pid = control.pid.load(Ordering::Relaxed);
            if pid > 0 { terminate_preview_process(pid); }
        }
        self.playback_texture = None;
    }

    fn start_playback(&mut self) {
        if self.playback_worker.is_some() { return; }
        let (Some(path), Some(info)) = (self.path.clone(), self.info) else { return; };
        if self.playhead_ms >= self.end_ms { self.playhead_ms = self.start_ms; }
        let duration_ms = self.end_ms.saturating_sub(self.playhead_ms);
        if duration_ms == 0 { return; }
        self.playback_error = None;
        self.playback_texture = None;
        self.playback_generation = self.playback_generation.wrapping_add(1);
        let generation = self.playback_generation;
        let filter = sticker_preview_filter(info, self.fit, self.left, self.top, self.zoom);
        let stop = Arc::new(AtomicBool::new(false));
        let pid = Arc::new(AtomicI32::new(0));
        self.playback_control = Some(PlaybackControl { stop: stop.clone(), pid: pid.clone() });
        let tx = self.playback_tx.clone();
        self.is_playing = true;
        self.playback_worker = Some(thread::spawn(move || {
            stream_preview(
                path, self::PLAYBACK_WIDTH, self::PLAYBACK_HEIGHT, self::PLAYBACK_FPS,
                self.start_ms, duration_ms, filter, generation, stop, pid, tx,
            );
        }));
    }

    pub(super) fn render(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Trim the clip, preview the result, then position the square sticker frame.").size(12.5).color(Color32::from_rgb(176, 196, 211)));
        ui.add_space(7.0);
        if let Some(error) = &self.error {
            ui.colored_label(Color32::from_rgb(239, 125, 139), error);
            return;
        }
        let Some(info) = self.info else {
            ui.horizontal(|ui| { ui.spinner(); ui.label("Analyzing video with FFprobe…"); });
            return;
        };

        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!("{} × {}", info.width, info.height)).size(11.5).color(Color32::from_rgb(176, 196, 211)));
            ui.separator();
            ui.label(RichText::new(format!("Source {}", time_label(info.duration_ms))).size(11.5).color(Color32::from_rgb(176, 196, 211)));
            ui.separator();
            ui.label(RichText::new("512 × 512 · VP9 · WebM").size(11.5).color(Color32::from_rgb(72, 190, 218)));
        });
        ui.add_space(8.0);
        self.draw_preview(ui, info);
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            let can_start = self.playback_worker.is_none() && !self.is_playing;
            let label = if self.is_playing { "Ⅱ  Pause" } else if self.playback_worker.is_some() { "Stopping…" } else { "▶  Play preview" };
            let enabled = self.is_playing || can_start;
            if ui.add_enabled(enabled, egui::Button::new(RichText::new(label).strong()).min_size(Vec2::new(132.0, 34.0)).fill(Color32::from_rgb(24, 91, 126))).clicked() {
                if self.is_playing { self.stop_playback(); } else { self.start_playback(); }
            }
            ui.label(RichText::new(format!("{} / {}", time_label(self.playhead_ms), time_label(self.end_ms))).monospace().size(12.0).color(Color32::from_rgb(176, 196, 211)));
            if let Some(error) = &self.playback_error {
                ui.colored_label(Color32::from_rgb(239, 125, 139), error);
            }
        });

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("TRIM CLIP").strong().size(11.0).color(Color32::from_rgb(72, 190, 218)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(format!("{:.2}s selected · 3.00s maximum", (self.end_ms - self.start_ms) as f32 / 1_000.0)).monospace().size(11.0).color(Color32::from_rgb(176, 196, 211)));
            });
        });
        self.draw_timeline(ui, info);
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("IN  {}", time_label(self.start_ms))).monospace().size(11.0).color(Color32::from_rgb(72, 190, 218)));
            ui.separator();
            ui.label(RichText::new(format!("OUT  {}", time_label(self.end_ms))).monospace().size(11.0).color(Color32::from_rgb(72, 190, 218)));
        });

        ui.add_space(14.0);
        ui.label(RichText::new("FRAME").strong().size(11.0).color(Color32::from_rgb(72, 190, 218)));
        ui.add_space(4.0);
        let previous_fit = self.fit;
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.fit, VideoStickerFit::Crop, "Fill · Crop");
            ui.selectable_value(&mut self.fit, VideoStickerFit::Fit, "Fit whole frame");
        });
        if self.fit != previous_fit { self.stop_playback(); }
        if self.fit == VideoStickerFit::Crop {
            ui.label(RichText::new("Drag the square to move it. Drag any corner to resize it while keeping the square shape.").size(11.0).color(Color32::from_rgb(150, 173, 191)));
        } else {
            ui.label(RichText::new("The entire source frame remains visible; empty space is transparent.").size(11.0).color(Color32::from_rgb(150, 173, 191)));
        }
        ui.add_space(7.0);
        ui.label(RichText::new("Telegram constraints are checked on export: 3 seconds, 30 FPS, no audio, maximum 256 KB.").size(11.0).color(Color32::from_rgb(150, 173, 191)));
    }

    fn draw_preview(&mut self, ui: &mut egui::Ui, info: VideoInfo) {
        if self.is_playing {
            let desired = 320.0_f32.min(ui.available_width().max(220.0));
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(desired), Sense::hover());
            ui.painter().rect_filled(rect, 8.0, Color32::from_rgb(5, 15, 26));
            if let Some(texture) = self.playback_texture.as_ref() {
                ui.painter().image(texture.id(), rect.shrink(2.0), egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
            } else {
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "Starting preview…", FontId::proportional(12.0), Color32::from_rgb(145, 169, 188));
            }
            return;
        }

        let Some(texture) = self.texture.as_ref() else {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().min(640.0), 250.0), Sense::hover());
            ui.painter().rect_filled(rect, 7.0, Color32::from_rgb(5, 15, 26));
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "Rendering preview frame…", FontId::proportional(13.0), Color32::from_rgb(145, 169, 188));
            if let Some(error) = &self.frame_error { ui.colored_label(Color32::from_rgb(239, 125, 139), error); }
            return;
        };

        let source_size = texture.size_vec2();
        let scale = (ui.available_width().min(680.0).max(220.0) / source_size.x).min(370.0 / source_size.y);
        let display_size = source_size * scale;
        let (image_rect, _) = ui.allocate_exact_size(display_size, Sense::hover());
        ui.painter().rect_filled(image_rect, 5.0, Color32::from_rgb(5, 15, 26));
        ui.painter().image(texture.id(), image_rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);

        if self.fit == VideoStickerFit::Crop {
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
            ui.painter().rect_stroke(crop, 2.0, Stroke::new(2.0, Color32::from_rgb(72, 190, 218)));
            self.draw_crop_handles(ui, image_rect, crop, info);
        } else {
            let fitted = egui::Rect::from_center_size(image_rect.center(), Vec2::splat(image_rect.width().min(image_rect.height())));
            ui.painter().rect_stroke(fitted, 2.0, Stroke::new(2.0, Color32::from_rgb(72, 190, 218)));
            ui.painter().text(fitted.center(), egui::Align2::CENTER_CENTER, "FIT", FontId::proportional(10.0), Color32::from_rgb(72, 190, 218));
        }
        if let Some(error) = &self.frame_error { ui.colored_label(Color32::from_rgb(239, 125, 139), error); }
    }

    fn draw_crop_handles(&mut self, ui: &mut egui::Ui, image_rect: egui::Rect, crop: egui::Rect, info: VideoInfo) {
        let corners = [crop.left_top(), crop.right_top(), crop.left_bottom(), crop.right_bottom()];
        let mut geometry_changed = false;

        let center = ui.interact(
            crop.shrink(CROP_HANDLE_SIZE * 0.75),
            ui.id().with("sticker-crop-move"),
            Sense::drag(),
        );
        if center.dragged() {
            self.crop_drag = Some(CropDrag::Move);
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
            geometry_changed = true;
        }

        for (index, point) in corners.iter().copied().enumerate() {
            let handle_rect = egui::Rect::from_center_size(point, Vec2::splat(CROP_HANDLE_SIZE));
            let response = ui.interact(handle_rect, ui.id().with(("sticker-crop-corner", index)), Sense::drag());
            ui.painter().rect_filled(
                egui::Rect::from_center_size(point, Vec2::splat(10.0)),
                2.0,
                Color32::from_rgb(72, 190, 218),
            );
            ui.painter().rect_stroke(
                egui::Rect::from_center_size(point, Vec2::splat(10.0)),
                2.0,
                Stroke::new(1.0, Color32::WHITE),
            );
            if response.dragged() {
                self.crop_drag = Some(CropDrag::Resize(index));
                if let Some(pointer) = ui.input(|input| input.pointer.interact_pos()) {
                    self.resize_crop_from_corner(info, image_rect, index, pointer);
                    geometry_changed = true;
                }
            }
        }

        if geometry_changed { self.stop_playback(); }
        if ui.input(|input| input.pointer.any_released()) { self.crop_drag = None; }
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
            .fill(Color32::from_rgb(6, 20, 34))
            .stroke(Stroke::new(1.0, Color32::from_rgb(24, 76, 106)))
            .rounding(7.0)
            .inner_margin(egui::Margin::symmetric(10.0, 8.0))
            .show(ui, |ui| {
                let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 62.0), Sense::click_and_drag());
                let track_left = rect.left() + 8.0;
                let track_right = rect.right() - 8.0;
                let track_width = (track_right - track_left).max(1.0);
                let x_at = |ms: u32| track_left + track_width * ms as f32 / info.duration_ms.max(1) as f32;

                let ruler_y = rect.top() + 7.0;
                for tick in 0..=4 {
                    let x = track_left + track_width * tick as f32 / 4.0;
                    ui.painter().line_segment(
                        [egui::pos2(x, ruler_y + 2.0), egui::pos2(x, ruler_y + 9.0)],
                        Stroke::new(1.0, Color32::from_rgb(72, 100, 121)),
                    );
                    let time = info.duration_ms.saturating_mul(tick) / 4;
                    ui.painter().text(
                        egui::pos2(x, ruler_y - 1.0),
                        if tick == 0 { egui::Align2::LEFT_BOTTOM } else if tick == 4 { egui::Align2::RIGHT_BOTTOM } else { egui::Align2::CENTER_BOTTOM },
                        time_label(time),
                        FontId::proportional(9.5),
                        Color32::from_rgb(113, 143, 163),
                    );
                }

                let track = egui::Rect::from_min_max(
                    egui::pos2(track_left, rect.top() + 26.0),
                    egui::pos2(track_right, rect.top() + 43.0),
                );
                ui.painter().rect_filled(track, 4.0, Color32::from_rgb(21, 39, 54));
                let selected = egui::Rect::from_min_max(
                    egui::pos2(x_at(self.start_ms), track.top()),
                    egui::pos2(x_at(self.end_ms), track.bottom()),
                );
                ui.painter().rect_filled(selected, 3.0, Color32::from_rgb(31, 112, 149));
                ui.painter().rect_stroke(selected, 3.0, Stroke::new(1.0, Color32::from_rgb(99, 207, 234)));

                for x in [x_at(self.start_ms), x_at(self.end_ms)] {
                    ui.painter().line_segment(
                        [egui::pos2(x, track.top() - 5.0), egui::pos2(x, track.bottom() + 5.0)],
                        Stroke::new(2.0, Color32::from_rgb(99, 207, 234)),
                    );
                    ui.painter().circle_filled(egui::pos2(x, track.center().y), 5.0, Color32::from_rgb(99, 207, 234));
                    ui.painter().circle_stroke(egui::pos2(x, track.center().y), 5.0, Stroke::new(1.0, Color32::WHITE));
                }

                let play_x = x_at(self.playhead_ms);
                ui.painter().line_segment(
                    [egui::pos2(play_x, rect.top() + 12.0), egui::pos2(play_x, rect.bottom() - 3.0)],
                    Stroke::new(2.0, Color32::from_rgb(247, 194, 95)),
                );
                ui.painter().circle_filled(egui::pos2(play_x, rect.top() + 13.0), 4.0, Color32::from_rgb(247, 194, 95));

                if response.drag_started() {
                    if let Some(pointer) = ui.input(|input| input.pointer.interact_pos()) {
                        let start_distance = (pointer.x - x_at(self.start_ms)).abs();
                        let end_distance = (pointer.x - x_at(self.end_ms)).abs();
                        self.timeline_drag = Some(if start_distance <= 16.0 && start_distance <= end_distance {
                            TimelineDrag::Start
                        } else if end_distance <= 16.0 {
                            TimelineDrag::End
                        } else {
                            TimelineDrag::Playhead
                        });
                    }
                }
                if response.dragged() || response.clicked() {
                    if let Some(pointer) = ui.input(|input| input.pointer.interact_pos()) {
                        let fraction = ((pointer.x - track_left) / track_width).clamp(0.0, 1.0);
                        let value = (fraction * info.duration_ms as f32).round() as u32;
                        let old_start = self.start_ms;
                        let old_end = self.end_ms;
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
                                self.playhead_ms = value.clamp(self.start_ms, self.end_ms);
                            }
                        }
                        if old_start != self.start_ms || old_end != self.end_ms {
                            self.stop_playback();
                            self.playhead_ms = self.playhead_ms.clamp(self.start_ms, self.end_ms);
                        }
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

fn sticker_preview_filter(info: VideoInfo, fit: VideoStickerFit, left: u16, top: u16, zoom: f32) -> String {
    match fit {
        VideoStickerFit::Fit => {
            "scale=512:512:force_original_aspect_ratio=decrease:force_divisible_by=2:flags=lanczos,pad=512:512:(ow-iw)/2:(oh-ih)/2:color=black@0,setsar=1,fps=24,format=rgba".into()
        }
        VideoStickerFit::Crop => {
            let minimum_side = (info.width.min(info.height) & !1).max(2);
            let side = ((minimum_side as f32 / zoom.clamp(1.0, 4.0)) as u32 & !1).max(2);
            let x = ((info.width.saturating_sub(side) as u64 * u64::from(left.min(1_000))) / 1_000) as u32 & !1;
            let y = ((info.height.saturating_sub(side) as u64 * u64::from(top.min(1_000))) / 1_000) as u32 & !1;
            format!("crop={side}:{side}:{x}:{y},scale=512:512:flags=lanczos,setsar=1,fps=24,format=rgba")
        }
    }
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

fn inspect_video(path: &Path) -> Result<VideoInfo, String> {
    let output = Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height,duration:format=duration", "-of", "json"])
        .arg(path)
        .output()
        .map_err(|e| format!("Could not start FFprobe: {e}. Install FFmpeg and FFprobe."))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(if detail.is_empty() { "FFprobe could not read the selected video.".into() } else { detail });
    }
    let value: Value = serde_json::from_slice(&output.stdout).map_err(|e| format!("FFprobe returned invalid metadata: {e}"))?;
    let stream = value["streams"].as_array().and_then(|s| s.first()).ok_or("The selected file does not contain a video stream.")?;
    let duration = value["format"]["duration"].as_str().and_then(|s| s.parse::<f64>().ok())
        .or_else(|| stream["duration"].as_str().and_then(|s| s.parse::<f64>().ok())).unwrap_or(0.0);
    let width = stream["width"].as_u64().unwrap_or(0) as u32;
    let height = stream["height"].as_u64().unwrap_or(0) as u32;
    if width < 2 || height < 2 || !duration.is_finite() || duration <= 0.0 {
        return Err("FFprobe could not determine the video dimensions or duration.".into());
    }
    Ok(VideoInfo { width, height, duration_ms: (duration * 1_000.0).round().clamp(1.0, u32::MAX as f64) as u32 })
}

fn preview_frame(path: &Path, time_ms: u32) -> Result<egui::ColorImage, String> {
    let seek = format!("{:.3}", time_ms as f64 / 1_000.0);
    let output = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-ss", &seek, "-i"])
        .arg(path)
        .args(["-frames:v", "1", "-vf", "scale=960:960:force_original_aspect_ratio=decrease:flags=lanczos", "-f", "image2pipe", "-vcodec", "png", "pipe:1"])
        .output()
        .map_err(|e| format!("Could not start FFmpeg preview: {e}"))?;
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
    use crate::engine::VideoStickerFit;
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
    fn playback_filters_match_square_crop_and_fit_modes() {
        let info = VideoInfo { width: 1920, height: 1080, duration_ms: 10_000 };
        let crop = sticker_preview_filter(info, VideoStickerFit::Crop, 500, 500, 2.0);
        assert!(crop.starts_with("crop=540:540:690:270,scale=512:512"));
        assert!(crop.contains("fps=24,format=rgba"));
        let fit = sticker_preview_filter(info, VideoStickerFit::Fit, 500, 500, 1.0);
        assert!(fit.contains("pad=512:512"));
        assert!(fit.contains("color=black@0"));
    }
}
