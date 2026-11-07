use crate::engine::{JobOptions, VideoStickerFit};
use eframe::egui::{self, Color32, FontId, RichText, Sense, Stroke, TextureHandle, Vec2};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

#[derive(Clone, Copy)]
struct VideoInfo { width: u32, height: u32, duration_ms: u32 }
enum Message { Metadata(PathBuf, Result<VideoInfo, String>), Frame(PathBuf, u32, Result<egui::ColorImage, String>) }
#[derive(Clone, Copy)]
enum Drag { Start, End, Playhead }

pub(super) struct VideoStickerEditor {
    path: Option<PathBuf>, info: Option<VideoInfo>, error: Option<String>, frame_error: Option<String>,
    start_ms: u32, end_ms: u32, playhead_ms: u32, fit: VideoStickerFit,
    left: u16, top: u16, zoom: f32, texture: Option<TextureHandle>, texture_time: Option<u32>,
    metadata_pending: Option<PathBuf>, frame_pending: Option<(PathBuf, u32)>,
    drag: Option<Drag>, tx: Sender<Message>, rx: Receiver<Message>, workers: Vec<JoinHandle<()>>,
}

impl Default for VideoStickerEditor {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self { path: None, info: None, error: None, frame_error: None,
            start_ms: 0, end_ms: 0, playhead_ms: 0, fit: VideoStickerFit::Crop,
            left: 500, top: 500, zoom: 1.0, texture: None, texture_time: None,
            metadata_pending: None, frame_pending: None, drag: None, tx, rx, workers: Vec::new() }
    }
}

impl VideoStickerEditor {
    pub(super) fn set_input(&mut self, path: Option<PathBuf>) {
        if self.path == path { return; }
        self.path = path.clone();
        self.info = None; self.error = None; self.frame_error = None;
        self.start_ms = 0; self.end_ms = 0; self.playhead_ms = 0;
        self.fit = VideoStickerFit::Crop; self.left = 500; self.top = 500; self.zoom = 1.0;
        self.texture = None; self.texture_time = None;
        self.metadata_pending = None; self.frame_pending = None; self.drag = None;
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
                                image, egui::TextureOptions::LINEAR,
                            ));
                            self.texture_time = Some(time_ms);
                            self.frame_error = None;
                        }
                        Err(error) => self.frame_error = Some(error),
                    }
                }
            }
        }
        let mut index = 0;
        while index < self.workers.len() {
            if self.workers[index].is_finished() {
                let worker = self.workers.swap_remove(index);
                let _ = worker.join();
            } else { index += 1; }
        }
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
        if self.metadata_pending.is_some() || self.frame_pending.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    pub(super) fn job_options(&self) -> Result<JobOptions, String> {
        let info = self.info.ok_or_else(|| "Wait until ForgeBox has analyzed the selected video.".to_string())?;
        if self.end_ms <= self.start_ms || self.end_ms - self.start_ms > 3_000 {
            return Err("Choose a valid segment no longer than three seconds.".into());
        }
        if self.end_ms > info.duration_ms { return Err("The selected segment extends beyond the source video.".into()); }
        Ok(JobOptions::VideoSticker {
            start_ms: self.start_ms, end_ms: self.end_ms, fit: self.fit,
            crop_left: self.left, crop_top: self.top,
            zoom_milli: (self.zoom.clamp(1.0, 4.0) * 1_000.0).round() as u16,
        })
    }

    pub(super) fn render(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Trim the clip, then position the square sticker frame.").size(12.5).color(Color32::from_rgb(176, 196, 211)));
        ui.add_space(7.0);
        if let Some(error) = &self.error { ui.colored_label(Color32::from_rgb(239, 125, 139), error); return; }
        let Some(info) = self.info else {
            ui.horizontal(|ui| { ui.spinner(); ui.label("Analyzing video with FFprobe…"); });
            return;
        };
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!("{} × {}", info.width, info.height)).size(11.5).color(Color32::from_rgb(176, 196, 211)));
            ui.separator();
            ui.label(RichText::new(format!("Source length: {}", time_label(info.duration_ms))).size(11.5).color(Color32::from_rgb(176, 196, 211)));
            ui.separator();
            ui.label(RichText::new("Output: 512 × 512 · VP9 · WebM").size(11.5).color(Color32::from_rgb(72, 190, 218)));
        });
        ui.add_space(7.0);
        self.draw_preview(ui, info);
        ui.add_space(10.0);
        ui.label(RichText::new("TRIM · UP TO 3 SECONDS").strong().size(11.0).color(Color32::from_rgb(72, 190, 218)));
        self.draw_timeline(ui, info);
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("In  {}", time_label(self.start_ms)));
            ui.label(RichText::new("→").color(Color32::from_rgb(100, 142, 165)));
            ui.label(format!("Out  {}", time_label(self.end_ms)));
            ui.separator();
            ui.label(RichText::new(format!("Selected {:.2}s / 3.00s", (self.end_ms - self.start_ms) as f32 / 1_000.0)).strong().color(Color32::from_rgb(72, 190, 218)));
        });
        ui.add(egui::Slider::new(&mut self.playhead_ms, self.start_ms..=self.end_ms)
            .text("Preview position")
            .custom_formatter(|v, _| time_label(v.max(0.0) as u32)));
        ui.add_space(8.0);
        ui.label(RichText::new("SQUARE FRAMING").strong().size(11.0).color(Color32::from_rgb(72, 190, 218)));
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.fit, VideoStickerFit::Crop, "Fill square · Crop");
            ui.selectable_value(&mut self.fit, VideoStickerFit::Fit, "Fit whole frame");
        });
        match self.fit {
            VideoStickerFit::Crop => {
                ui.add(egui::Slider::new(&mut self.zoom, 1.0..=4.0).text("Zoom").custom_formatter(|v, _| format!("{v:.1}×")));
                ui.label(RichText::new("Drag the square in the preview to reposition the crop. Adjust Zoom to change its size.").size(11.0).color(Color32::from_rgb(150, 173, 191)));
            }
            VideoStickerFit::Fit => {
                ui.label(RichText::new("The entire source frame remains visible; empty space is transparent.").size(11.0).color(Color32::from_rgb(150, 173, 191)));
            }
        }
        ui.add_space(8.0);
        ui.label(RichText::new("Telegram: maximum 3 seconds, 30 FPS, no audio and 256 KB. ForgeBox checks the exported file.").size(11.0).color(Color32::from_rgb(150, 173, 191)));
    }

    fn draw_preview(&mut self, ui: &mut egui::Ui, info: VideoInfo) {
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

        let crop_rect = if self.fit == VideoStickerFit::Crop {
            let side = info.width.min(info.height) as f32 / self.zoom.clamp(1.0, 4.0);
            let cw = image_rect.width() * side / info.width as f32;
            let ch = image_rect.height() * side / info.height as f32;
            let x = image_rect.left() + (image_rect.width() - cw) * self.left as f32 / 1_000.0;
            let y = image_rect.top() + (image_rect.height() - ch) * self.top as f32 / 1_000.0;
            egui::Rect::from_min_size(egui::pos2(x, y), Vec2::new(cw, ch))
        } else {
            egui::Rect::from_center_size(image_rect.center(), Vec2::splat(image_rect.width().min(image_rect.height())))
        };

        if self.fit == VideoStickerFit::Crop {
            let shade = Color32::from_black_alpha(145);
            for r in [
                egui::Rect::from_min_max(image_rect.min, egui::pos2(image_rect.right(), crop_rect.top())),
                egui::Rect::from_min_max(egui::pos2(image_rect.left(), crop_rect.bottom()), image_rect.max),
                egui::Rect::from_min_max(egui::pos2(image_rect.left(), crop_rect.top()), egui::pos2(crop_rect.left(), crop_rect.bottom())),
                egui::Rect::from_min_max(egui::pos2(crop_rect.right(), crop_rect.top()), egui::pos2(image_rect.right(), crop_rect.bottom())),
            ] { if r.is_positive() { ui.painter().rect_filled(r, 0.0, shade); } }
        }
        ui.painter().rect_stroke(crop_rect, 1.0, Stroke::new(2.0, Color32::from_rgb(72, 190, 218)));
        for point in [crop_rect.left_top(), crop_rect.right_top(), crop_rect.left_bottom(), crop_rect.right_bottom()] {
            ui.painter().circle_filled(point, 5.5, Color32::from_rgb(72, 190, 218));
        }
        if self.fit == VideoStickerFit::Crop {
            let response = ui.interact(crop_rect, ui.id().with("sticker-crop-drag"), Sense::drag());
            if response.dragged() {
                let delta = ui.input(|i| i.pointer.delta());
                let side = info.width.min(info.height) as f32 / self.zoom.clamp(1.0, 4.0);
                let avail_x = image_rect.width() * (info.width as f32 - side) / info.width as f32;
                let avail_y = image_rect.height() * (info.height as f32 - side) / info.height as f32;
                if avail_x > 0.0 { self.left = (self.left as f32 + delta.x / avail_x * 1_000.0).round().clamp(0.0, 1_000.0) as u16; }
                if avail_y > 0.0 { self.top = (self.top as f32 + delta.y / avail_y * 1_000.0).round().clamp(0.0, 1_000.0) as u16; }
            }
        } else {
            ui.painter().text(crop_rect.center(), egui::Align2::CENTER_CENTER, "FIT", FontId::proportional(10.0), Color32::from_rgb(72, 190, 218));
        }
        if let Some(error) = &self.frame_error { ui.colored_label(Color32::from_rgb(239, 125, 139), error); }
    }

    fn draw_timeline(&mut self, ui: &mut egui::Ui, info: VideoInfo) {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width().max(220.0), 44.0), Sense::click_and_drag());
        let x_at = |ms: u32| rect.left() + rect.width() * ms as f32 / info.duration_ms.max(1) as f32;
        let track = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.center().y - 5.0), egui::pos2(rect.right(), rect.center().y + 5.0));
        ui.painter().rect_filled(track, 4.0, Color32::from_rgb(23, 43, 59));
        let selected = egui::Rect::from_min_max(egui::pos2(x_at(self.start_ms), track.top()), egui::pos2(x_at(self.end_ms), track.bottom()));
        ui.painter().rect_filled(selected, 4.0, Color32::from_rgb(37, 115, 151));
        for x in [x_at(self.start_ms), x_at(self.end_ms)] {
            let thumb = egui::Rect::from_center_size(egui::pos2(x, track.center().y), Vec2::new(8.0, 27.0));
            ui.painter().rect_filled(thumb, 2.0, Color32::from_rgb(92, 205, 231));
            ui.painter().rect_stroke(thumb, 2.0, Stroke::new(1.0, Color32::WHITE));
        }
        let play_x = x_at(self.playhead_ms);
        ui.painter().line_segment([egui::pos2(play_x, rect.top() + 1.0), egui::pos2(play_x, rect.bottom() - 1.0)], Stroke::new(2.0, Color32::from_rgb(247, 201, 111)));
        if response.drag_started() {
            if let Some(p) = ui.input(|i| i.pointer.interact_pos()) {
                let ds = (p.x - x_at(self.start_ms)).abs();
                let de = (p.x - x_at(self.end_ms)).abs();
                self.drag = Some(if ds <= 14.0 && ds <= de { Drag::Start } else if de <= 14.0 { Drag::End } else { Drag::Playhead });
            }
        }
        if response.dragged() || response.clicked() {
            if let Some(p) = ui.input(|i| i.pointer.interact_pos()) {
                let value = (((p.x - rect.left()) / rect.width().max(1.0)).clamp(0.0, 1.0) * info.duration_ms as f32).round() as u32;
                match self.drag.unwrap_or(Drag::Playhead) {
                    Drag::Start => {
                        self.start_ms = value.min(self.end_ms.saturating_sub(100));
                        if self.end_ms.saturating_sub(self.start_ms) > 3_000 { self.end_ms = (self.start_ms + 3_000).min(info.duration_ms); }
                    }
                    Drag::End => self.end_ms = value.clamp(self.start_ms + 100, (self.start_ms + 3_000).min(info.duration_ms)),
                    Drag::Playhead => self.playhead_ms = value.clamp(self.start_ms, self.end_ms),
                }
                self.playhead_ms = self.playhead_ms.clamp(self.start_ms, self.end_ms);
            }
        }
        if response.drag_stopped() { self.drag = None; }
    }
}

fn inspect_video(path: &Path) -> Result<VideoInfo, String> {
    let output = Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height,duration:format=duration", "-of", "json"])
        .arg(path).output()
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
        .output().map_err(|e| format!("Could not start FFmpeg preview: {e}"))?;
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
