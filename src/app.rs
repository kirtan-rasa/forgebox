use crate::catalog::{info, Category, ToolId};
use crate::engine::{
    execute, take_execution_warnings, AudioFormat, DataFormat, ImageFormat, Job, JobOptions,
    PdfCompressionProfile, PdfImageFormat, PdfRotation, VideoFormat, VideoPreset,
};
use eframe::egui::{self, Vec2};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

mod theme;
mod view;

struct JobResult {
    result: Result<Vec<PathBuf>, String>,
    warnings: Vec<String>,
}

enum DialogResult {
    Files(Option<Vec<PathBuf>>),
    Folder(Option<PathBuf>),
    Secondary(Option<PathBuf>),
}

struct ForgeApp {
    selected_tool: ToolId,
    icons_loaded: bool,
    active_category: Option<Category>,
    search: String,
    files: Vec<PathBuf>,
    selection_notice: Option<String>,
    secondary_file: Option<PathBuf>,
    output_dir: PathBuf,
    target_format: String,
    text: String,
    width: String,
    height: String,
    quality: String,
    compress_pdf: bool,
    pdf_profile: String,
    rotation: String,
    pages: String,
    fps: String,
    video_preset: String,
    language: String,
    dpi: String,
    running: bool,
    wizard_step: usize,
    dragging_file: Option<PathBuf>,
    drag_insert_index: Option<usize>,
    drag_card_size: Option<Vec2>,
    dialog_open: bool,
    thumbnails: HashMap<PathBuf, egui::TextureHandle>,
    thumbnail_errors: HashSet<PathBuf>,
    thumbnail_pending: HashSet<PathBuf>,
    logs: Vec<(bool, String)>,
    tx: Sender<JobResult>,
    rx: Receiver<JobResult>,
    dialog_tx: Sender<DialogResult>,
    dialog_rx: Receiver<DialogResult>,
    thumbnail_tx: Sender<(PathBuf, Result<egui::ColorImage, String>)>,
    thumbnail_rx: Receiver<(PathBuf, Result<egui::ColorImage, String>)>,
}

impl ForgeApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::apply(&cc.egui_ctx);
        let icons_loaded = view::load_nerd_font(&cc.egui_ctx);
        let (tx, rx) = mpsc::channel();
        let (dialog_tx, dialog_rx) = mpsc::channel();
        let (thumbnail_tx, thumbnail_rx) = mpsc::channel();
        Self {
            selected_tool: ToolId::ImageConvert,
            icons_loaded,
            active_category: None,
            search: String::new(),
            files: Vec::new(),
            selection_notice: None,
            secondary_file: None,
            output_dir: view::default_output_dir(),
            target_format: "png".into(),
            text: String::new(),
            width: "1600".into(),
            height: "1200".into(),
            quality: "82".into(),
            compress_pdf: false,
            pdf_profile: "ebook".into(),
            rotation: "90".into(),
            pages: "1,last".into(),
            fps: "12".into(),
            video_preset: "fast".into(),
            language: "eng".into(),
            dpi: "150".into(),
            running: false,
            wizard_step: 0,
            dragging_file: None,
            drag_insert_index: None,
            drag_card_size: None,
            dialog_open: false,
            thumbnails: HashMap::new(),
            thumbnail_errors: HashSet::new(),
            thumbnail_pending: HashSet::new(),
            logs: Vec::new(),
            tx,
            rx,
            dialog_tx,
            dialog_rx,
            thumbnail_tx,
            thumbnail_rx,
        }
    }

    fn select_tool(&mut self, id: ToolId) {
        self.selected_tool = id;
        self.secondary_file = None;
        self.files.clear();
        self.selection_notice = None;
        self.wizard_step = 0;
        self.dragging_file = None;
        self.drag_insert_index = None;
        self.drag_card_size = None;
        self.logs.clear();
        match id {
            ToolId::ImageConvert => self.target_format = "png".into(),
            ToolId::ImagesToPdf => self.target_format = "pdf".into(),
            ToolId::PdfToImages => { self.target_format = "png".into(); self.dpi = "150".into(); },
            ToolId::ExtractAudio => self.target_format = "mp3".into(),
            ToolId::VideoConvert => { self.target_format = "mp4".into(); self.quality = "23".into(); self.video_preset = "fast".into(); },
            ToolId::DataConvert => self.target_format = "json".into(),
            ToolId::VideoToGif => { self.width = "480".into(); self.fps = "12".into(); },
            ToolId::ImageResize => { self.width = "1600".into(); self.height = "1200".into(); },
            ToolId::ImageCompress => self.quality = "82".into(),
            ToolId::PdfRotate => self.rotation = "90".into(),
            ToolId::PdfCompress => self.pdf_profile = "ebook".into(),
            ToolId::PdfSplit => self.pages = "1,last".into(),
            ToolId::PdfReorder => self.pages.clear(),
            _ => {}
        }
    }

    fn is_single_file_tool(&self) -> bool {
        matches!(self.selected_tool,
            ToolId::PdfSplit | ToolId::PdfCompress | ToolId::PdfRotate | ToolId::PdfReorder |
            ToolId::PdfToImages | ToolId::PdfWatermark | ToolId::DataConvert | ToolId::LongPngToPdf
        )
    }

    fn can_advance_from_files(&self) -> bool {
        match self.selected_tool {
            ToolId::GenerateQr => true,
            ToolId::PdfMerge => self.files.len() >= 2,
            ToolId::PdfWatermark => !self.files.is_empty() && self.secondary_file.is_some(),
            _ => !self.files.is_empty(),
        }
    }

    /// Whether the selected tool has user-configurable options that deserve
    /// their own wizard step. File-only tools should go straight to saving.
    fn has_parameter_step(&self) -> bool {
        !matches!(self.selected_tool,
            ToolId::ImageGrayscale
                | ToolId::RemoveBackground
                | ToolId::ExtractExif
                | ToolId::LongPngToPdf
                | ToolId::PdfMerge
                | ToolId::PdfWatermark
                | ToolId::DocToPdf
                | ToolId::PdfToDoc
        )
    }

    /// Logical step IDs are kept stable: files/content = 0, parameters = 1,
    /// save/run = 2. The visible sequence simply omits step 1 when irrelevant.
    fn wizard_step_sequence(&self) -> Vec<usize> {
        if self.has_parameter_step() {
            vec![0, 1, 2]
        } else {
            vec![0, 2]
        }
    }

    fn add_files(&mut self, mut paths: Vec<PathBuf>) {
        if self.selected_tool == ToolId::GenerateQr { return; }
        self.selection_notice = None;

        let requested = paths.len();
        paths.retain(|path| path.is_file() && view::is_supported_input_file(self.selected_tool, path));
        let skipped = requested.saturating_sub(paths.len());
        let mut notices = Vec::new();
        if skipped > 0 {
            notices.push(format!(
                "Skipped {skipped} unsupported item(s). This tool accepts {}.",
                view::accepted_input_description(self.selected_tool)
            ));
        }

        if self.is_single_file_tool() {
            if paths.len() > 1 {
                notices.push("This tool accepts one file; only the first was selected.".into());
            }
            if let Some(path) = paths.into_iter().next() {
                self.files.clear();
                self.files.push(path);
            }
        } else {
            for path in paths {
                if !self.files.contains(&path) { self.files.push(path); }
            }
        }

        if !notices.is_empty() {
            self.selection_notice = Some(notices.join(" "));
        }
    }

    fn choose_files(&mut self) {
        if self.dialog_open { return; }
        let mut dialog = rfd::FileDialog::new().set_title("Select files to process");
        match self.selected_tool {
            ToolId::PdfMerge | ToolId::PdfSplit | ToolId::PdfCompress | ToolId::PdfRotate | ToolId::PdfReorder | ToolId::PdfToImages | ToolId::PdfWatermark | ToolId::PdfToDoc => { dialog = dialog.add_filter("PDF", &["pdf"]); }
            ToolId::DocToPdf => { dialog = dialog.add_filter("Documents", &["doc", "docx", "odt", "rtf"]); }
            ToolId::ImageOcr | ToolId::ExtractExif | ToolId::ImagesToPdf | ToolId::ImageConvert | ToolId::ImageResize | ToolId::ImageCompress | ToolId::ImageGrayscale | ToolId::RemoveBackground | ToolId::LongPngToPdf => { dialog = dialog.add_filter("Images", &["png", "jpg", "jpeg", "webp", "tif", "tiff", "bmp", "heic", "avif", "gif"]); }
            ToolId::DataConvert => { dialog = dialog.add_filter("Spreadsheets and data", &["csv", "json", "xlsx"]); }
            ToolId::VideoConvert | ToolId::VideoToGif => { dialog = dialog.add_filter("Videos", &["mp4", "mkv", "mov", "webm", "avi", "m4v", "mpeg", "mpg", "3gp", "ogv", "flv", "mts", "m2ts"]); }
            ToolId::ExtractAudio => { dialog = dialog.add_filter("Video and audio", &["mp4", "mkv", "mov", "webm", "avi", "m4v", "mpeg", "mpg", "3gp", "ogv", "flv", "mp3", "wav", "flac", "m4a", "ogg", "aac", "mts", "m2ts"]); }
            _ => {}
        }
        self.dialog_open = true;
        let tx = self.dialog_tx.clone();
        let single_file = self.is_single_file_tool();
        thread::spawn(move || {
            let paths = if single_file {
                dialog.pick_file().map(|path| vec![path])
            } else {
                dialog.pick_files()
            };
            let _ = tx.send(DialogResult::Files(paths));
        });
    }

    fn choose_output_folder(&mut self) {
        if self.dialog_open { return; }
        let dialog = rfd::FileDialog::new().set_title("Choose output folder").set_directory(&self.output_dir);
        self.dialog_open = true;
        let tx = self.dialog_tx.clone();
        thread::spawn(move || {
            let path = dialog.pick_folder();
            let _ = tx.send(DialogResult::Folder(path));
        });
    }

    fn choose_secondary_file(&mut self) {
        if self.dialog_open { return; }
        let dialog = rfd::FileDialog::new().set_title("Select watermark PDF").add_filter("PDF", &["pdf"]);
        self.dialog_open = true;
        let tx = self.dialog_tx.clone();
        thread::spawn(move || {
            let path = dialog.pick_file();
            let _ = tx.send(DialogResult::Secondary(path));
        });
    }

    fn poll_dialogs(&mut self) {
        while let Ok(result) = self.dialog_rx.try_recv() {
            self.dialog_open = false;
            match result {
                DialogResult::Files(Some(paths)) => {
                    self.add_files(paths);
                    if !self.files.is_empty() { self.wizard_step = 0; }
                }
                DialogResult::Folder(Some(path)) => self.output_dir = path,
                DialogResult::Secondary(Some(path)) => self.secondary_file = Some(path),
                DialogResult::Secondary(None) => {},
                DialogResult::Files(None) | DialogResult::Folder(None) => {}
            }
        }
    }

    fn queue_thumbnails(&mut self) {
        if self.selected_tool != ToolId::ImagesToPdf { return; }

        let available_slots = 2usize.saturating_sub(self.thumbnail_pending.len());
        if available_slots == 0 { return; }

        // Inspect the file list by reference and clone only the paths that can
        // actually be scheduled. Cloning the entire batch every repaint is
        // unnecessary, especially when a large set of pages is being added.
        let mut candidates = Vec::with_capacity(available_slots);
        for path in &self.files {
            if self.thumbnails.contains_key(path)
                || self.thumbnail_pending.contains(path)
                || self.thumbnail_errors.contains(path)
            {
                continue;
            }
            candidates.push(path.clone());
            if candidates.len() >= available_slots { break; }
        }

        for path in candidates {
            self.thumbnail_pending.insert(path.clone());
            let tx = self.thumbnail_tx.clone();
            thread::spawn(move || {
                let result = view::load_thumbnail(&path);
                let _ = tx.send((path, result));
            });
        }
    }

    fn poll_thumbnails(&mut self, ctx: &egui::Context) {
        while let Ok((path, result)) = self.thumbnail_rx.try_recv() {
            self.thumbnail_pending.remove(&path);
            if !self.files.contains(&path) { continue; }
            match result {
                Ok(image) => {
                    let texture = ctx.load_texture(
                        format!("thumb:{}", path.display()),
                        image,
                        egui::TextureOptions::LINEAR,
                    );
                    self.thumbnails.insert(path, texture);
                }
                Err(_) => { self.thumbnail_errors.insert(path); }
            }
        }
        self.thumbnails.retain(|path, _| self.files.contains(path));
        self.thumbnail_errors.retain(|path| self.files.contains(path));
        // Keep in-flight entries for removed files until their worker returns so rapid
        // edits cannot accidentally launch unlimited thumbnail decoders in parallel.
    }

    fn build_job(&self) -> Result<Job, String> {
        let positive_u32 = |value: &str, fallback: u32| {
            value.trim().parse::<u32>().ok().filter(|number| *number > 0).unwrap_or(fallback)
        };
        let positive_u8 = |value: &str, fallback: u8, min: u8, max: u8| {
            value.trim().parse::<u8>().ok().filter(|number| *number > 0).unwrap_or(fallback).clamp(min, max)
        };

        let options = match self.selected_tool {
            ToolId::ImageConvert => JobOptions::ImageConvert {
                format: ImageFormat::parse(&self.target_format)?,
            },
            ToolId::ImageResize => JobOptions::ImageResize {
                width: positive_u32(&self.width, 1600),
                height: positive_u32(&self.height, 1200),
            },
            ToolId::ImageCompress => JobOptions::ImageCompress {
                quality: positive_u8(&self.quality, 82, 1, 100),
            },
            ToolId::ImagesToPdf => JobOptions::ImagesToPdf {
                compress: self.compress_pdf,
                quality: positive_u8(&self.quality, 82, 20, 100),
            },
            ToolId::ImageOcr => JobOptions::ImageOcr {
                language: if self.language.trim().is_empty() { "eng".into() } else { self.language.trim().to_owned() },
            },
            ToolId::GenerateQr => JobOptions::GenerateQr { text: self.text.clone() },
            ToolId::PdfSplit => JobOptions::PdfSplit { range: self.pages.trim().to_owned() },
            ToolId::PdfCompress => JobOptions::PdfCompress {
                profile: PdfCompressionProfile::parse(&self.pdf_profile)?,
            },
            ToolId::PdfRotate => JobOptions::PdfRotate {
                rotation: PdfRotation::parse(&self.rotation)?,
            },
            ToolId::PdfReorder => JobOptions::PdfReorder { order: self.pages.trim().to_owned() },
            ToolId::PdfToImages => JobOptions::PdfToImages {
                format: PdfImageFormat::parse(&self.target_format)?,
                dpi: positive_u32(&self.dpi, 150).clamp(36, 600) as u16,
            },
            ToolId::PdfWatermark => JobOptions::PdfWatermark {
                watermark_file: self.secondary_file.clone().ok_or("Select the watermark PDF in the first step.")?,
            },
            ToolId::DataConvert => JobOptions::DataConvert {
                format: DataFormat::parse(&self.target_format)?,
            },
            ToolId::ExtractAudio => JobOptions::ExtractAudio {
                format: AudioFormat::parse(&self.target_format)?,
            },
            ToolId::VideoConvert => JobOptions::VideoConvert {
                format: VideoFormat::parse(&self.target_format)?,
                preset: VideoPreset::parse(&self.video_preset)?,
                quality: positive_u8(&self.quality, 23, 18, 30),
            },
            ToolId::VideoToGif => JobOptions::VideoToGif {
                width: positive_u32(&self.width, 480).clamp(64, 1280) as u16,
                fps: positive_u32(&self.fps, 12).clamp(1, 30) as u8,
            },
            ToolId::ImageGrayscale
            | ToolId::RemoveBackground
            | ToolId::ExtractExif
            | ToolId::LongPngToPdf
            | ToolId::PdfMerge
            | ToolId::DocToPdf
            | ToolId::PdfToDoc => JobOptions::None,
        };

        Job::new(
            self.selected_tool,
            self.files.clone(),
            self.output_dir.clone(),
            options,
        )
    }

    fn start_job(&mut self) {
        if self.running { return; }
        if !matches!(self.selected_tool, ToolId::GenerateQr) && self.files.is_empty() {
            self.logs.push((false, "Add an input file first.".into()));
            return;
        }
        if self.selected_tool == ToolId::PdfMerge && self.files.len() < 2 {
            self.logs.push((false, "Select at least two PDF files to merge.".into()));
            return;
        }
        if self.selected_tool == ToolId::PdfWatermark && self.secondary_file.is_none() {
            self.logs.push((false, "Select the watermark PDF in the first step.".into()));
            self.wizard_step = 0;
            return;
        }
        if self.selected_tool == ToolId::GenerateQr && self.text.trim().is_empty() {
            self.logs.push((false, "Enter text or a URL for the QR code in the Options step.".into()));
            self.wizard_step = 1;
            return;
        }
        let job = match self.build_job() {
            Ok(job) => job,
            Err(error) => {
                self.logs.push((false, error));
                return;
            }
        };
        let tx = self.tx.clone();
        let name = info(self.selected_tool).name.to_string();
        self.logs.clear();
        self.logs.push((true, format!("Processing: {name}")));
        self.running = true;
        thread::spawn(move || {
            let result = execute(job);
            let warnings = take_execution_warnings();
            let _ = tx.send(JobResult { result, warnings });
        });
    }

    fn poll_results(&mut self) {
        while let Ok(message) = self.rx.try_recv() {
            self.running = false;
            self.logs.clear();
            match message.result {
                Ok(paths) => {
                    if paths.is_empty() {
                        self.logs.push((true, "Done. The command completed successfully.".into()));
                    } else {
                        for path in paths {
                            self.logs.push((true, format!("Done: {}", path.display())));
                        }
                    }
                    for warning in message.warnings {
                        self.logs.push((true, format!("Warning: {warning}")));
                    }
                }
                Err(error) => self.logs.push((false, error)),
            }
            if self.logs.len() > 200 { self.logs.drain(0..self.logs.len() - 200); }
        }
    }

    fn can_run(&self) -> bool {
        if self.running { return false; }
        match self.selected_tool {
            ToolId::GenerateQr => !self.text.trim().is_empty(),
            ToolId::PdfMerge => self.files.len() >= 2,
            ToolId::PdfSplit | ToolId::PdfReorder => {
                !self.files.is_empty() && !self.pages.trim().is_empty()
            }
            ToolId::PdfWatermark => !self.files.is_empty() && self.secondary_file.is_some(),
            _ => !self.files.is_empty(),
        }
    }
}


pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1240.0, 900.0])
            .with_min_inner_size([720.0, 520.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "ForgeBox",
        options,
        Box::new(|cc| Ok(Box::new(ForgeApp::new(cc)))),
    )
}
