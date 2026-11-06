use crate::catalog::{info, Category, ToolId};
use crate::engine::{
    execute_with_control, take_execution_warnings, AudioFormat, DataFormat, ExecutionControl,
    ImageFormat, Job, JobOptions, PdfCompressionProfile, PdfImageFormat, PdfRotation,
    VideoFormat, VideoPreset,
};
use eframe::egui::{self, Vec2};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

mod theme;
mod view;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum QueueStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

struct QueuedJob {
    job: Job,
    tool_name: String,
    file_count: usize,
    status: QueueStatus,
    output_count: usize,
    summary: Option<String>,
    warnings: Vec<String>,
}

enum QueueEvent {
    Started { index: usize, total: usize, tool_name: String },
    Finished {
        index: usize,
        result: Result<Vec<PathBuf>, String>,
        warnings: Vec<String>,
        cancelled: bool,
    },
    Skipped { index: usize },
    QueueFinished { cancelled: bool },
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
    execution_control: Option<ExecutionControl>,
    queue_worker: Option<thread::JoinHandle<()>>,
    execution_status: String,
    status_tx: Sender<String>,
    status_rx: Receiver<String>,
    cancel_requested: bool,
    completed_cancelled: bool,
    active_task_name: String,
    active_queue_index: usize,
    queue_total: usize,
    queue_finished: usize,
    queue: Vec<QueuedJob>,
    queue_run_indices: Vec<usize>,
    last_output_dir: Option<PathBuf>,
    wizard_step: usize,
    dragging_file: Option<PathBuf>,
    drag_insert_index: Option<usize>,
    drag_card_size: Option<Vec2>,
    dialog_open: bool,
    thumbnails: HashMap<PathBuf, egui::TextureHandle>,
    thumbnail_errors: HashSet<PathBuf>,
    thumbnail_pending: HashSet<PathBuf>,
    thumbnail_control: ExecutionControl,
    thumbnail_workers: Vec<thread::JoinHandle<()>>,
    logs: Vec<(bool, String)>,
    queue_tx: Sender<QueueEvent>,
    queue_rx: Receiver<QueueEvent>,
    dialog_tx: Sender<DialogResult>,
    dialog_rx: Receiver<DialogResult>,
    thumbnail_tx: Sender<(PathBuf, Result<egui::ColorImage, String>)>,
    thumbnail_rx: Receiver<(PathBuf, Result<egui::ColorImage, String>)>,
}

impl ForgeApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::apply(&cc.egui_ctx);
        let icons_loaded = view::load_nerd_font(&cc.egui_ctx);
        let (queue_tx, queue_rx) = mpsc::channel();
        let (dialog_tx, dialog_rx) = mpsc::channel();
        let (thumbnail_tx, thumbnail_rx) = mpsc::channel();
        let (status_tx, status_rx) = mpsc::channel();
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
            execution_control: None,
            queue_worker: None,
            execution_status: String::new(),
            status_tx,
            status_rx,
            cancel_requested: false,
            completed_cancelled: false,
            active_task_name: String::new(),
            active_queue_index: 0,
            queue_total: 0,
            queue_finished: 0,
            queue: Vec::new(),
            queue_run_indices: Vec::new(),
            last_output_dir: None,
            wizard_step: 0,
            dragging_file: None,
            drag_insert_index: None,
            drag_card_size: None,
            dialog_open: false,
            thumbnails: HashMap::new(),
            thumbnail_errors: HashSet::new(),
            thumbnail_pending: HashSet::new(),
            thumbnail_control: ExecutionControl::silent(),
            thumbnail_workers: Vec::new(),
            logs: Vec::new(),
            queue_tx,
            queue_rx,
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
        self.completed_cancelled = false;
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
            let control = self.thumbnail_control.clone();
            self.thumbnail_workers.push(thread::spawn(move || {
                let result = view::load_thumbnail(&path, control);
                let _ = tx.send((path, result));
            }));
        }
    }

    fn poll_thumbnails(&mut self, ctx: &egui::Context) {
        // Reap workers after their result has been delivered. The worker list
        // stays bounded by the small thumbnail concurrency limit.
        let mut index = 0;
        while index < self.thumbnail_workers.len() {
            if self.thumbnail_workers[index].is_finished() {
                let worker = self.thumbnail_workers.swap_remove(index);
                let _ = worker.join();
            } else {
                index += 1;
            }
        }

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
            value.trim().parse::<i32>().ok().filter(|number| *number > 0).map(|number| number as u32).unwrap_or(fallback)
        };
        let positive_u8 = |value: &str, fallback: u8, min: u8, max: u8| {
            value.trim().parse::<i32>().ok().filter(|number| *number > 0).unwrap_or(i32::from(fallback)).clamp(i32::from(min), i32::from(max)) as u8
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
        if self.running {
            return;
        }
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

        let file_count = job.files.len();
        let tool_name = info(self.selected_tool).name.to_string();
        self.queue.push(QueuedJob {
            job,
            tool_name: tool_name.clone(),
            file_count,
            status: QueueStatus::Pending,
            output_count: 0,
            summary: None,
            warnings: Vec::new(),
        });

        // The job owns snapshots of all its inputs and options. Clear the editor
        // so another tool or file set can be prepared without altering queued work.
        self.files.clear();
        self.secondary_file = None;
        self.selection_notice = None;
        self.dragging_file = None;
        self.drag_insert_index = None;
        self.drag_card_size = None;
        self.wizard_step = 0;
        self.last_output_dir = None;
        self.logs.clear();
        self.logs.push((true, format!("Added to queue: {tool_name}")));
    }

    fn start_queue(&mut self) {
        if self.running || self.queue.is_empty() {
            return;
        }

        while self.status_rx.try_recv().is_ok() {}
        let jobs: Vec<(usize, Job, String)> = self.queue
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.status == QueueStatus::Pending)
            .map(|(index, entry)| (index, entry.job.clone(), entry.tool_name.clone()))
            .collect();
        if jobs.is_empty() {
            return;
        }

        // Queue execution is independent of the editor's current step.
        // Always show the live queue monitor while tasks are running.
        self.wizard_step = 2;
        self.last_output_dir = None;

        let total = jobs.len();
        let queue_tx = self.queue_tx.clone();
        let control = ExecutionControl::new(self.status_tx.clone());
        self.execution_control = Some(control.clone());
        self.execution_status = "Preparing queue…".into();
        self.active_task_name = "Queued tasks".into();
        self.active_queue_index = 0;
        self.queue_total = total;
        self.queue_finished = 0;
        self.queue_run_indices = jobs.iter().map(|(index, _, _)| *index).collect();
        self.cancel_requested = false;
        self.completed_cancelled = false;
        self.logs.clear();
        self.running = true;

        // A single worker processes queued tasks in order. A failure is recorded
        // but does not prevent later independent tasks from running.
        self.queue_worker = Some(thread::spawn(move || {
            for (position, (index, job, tool_name)) in jobs.iter().cloned().enumerate() {
                if control.is_cancelled() {
                    let _ = queue_tx.send(QueueEvent::Skipped { index });
                    continue;
                }

                let _ = queue_tx.send(QueueEvent::Started {
                    index,
                    total,
                    tool_name,
                });

                let result = execute_with_control(job, control.clone());
                let cancelled = control.is_cancelled()
                    && result.as_ref().err()
                        .map(|error| error.contains("Operation cancelled by user."))
                        .unwrap_or(false);
                let warnings = take_execution_warnings();
                let _ = queue_tx.send(QueueEvent::Finished {
                    index,
                    result,
                    warnings,
                    cancelled,
                });

                if cancelled {
                    for (pending_index, _, _) in jobs.iter().skip(position + 1) {
                        let _ = queue_tx.send(QueueEvent::Skipped { index: *pending_index });
                    }
                    break;
                }
            }

            let _ = queue_tx.send(QueueEvent::QueueFinished {
                cancelled: control.is_cancelled(),
            });
        }));
    }

    fn cancel_job(&mut self) {
        if let Some(control) = &self.execution_control {
            if !self.cancel_requested {
                control.cancel();
                self.cancel_requested = true;
                self.execution_status = "Stopping queue…".into();
            }
        }
    }

    fn clear_queue(&mut self) {
        if !self.running {
            self.queue.clear();
            self.queue_run_indices.clear();
            self.last_output_dir = None;
            self.logs.clear();
            self.completed_cancelled = false;
        }
    }

    fn poll_execution_status(&mut self) {
        while let Ok(status) = self.status_rx.try_recv() {
            self.execution_status = status;
        }
    }

    fn poll_queue_events(&mut self) {
        while let Ok(event) = self.queue_rx.try_recv() {
            match event {
                QueueEvent::Started { index, total, tool_name } => {
                    if let Some(entry) = self.queue.get_mut(index) {
                        entry.status = QueueStatus::Running;
                    }
                    self.active_queue_index = self.queue_finished;
                    self.queue_total = total;
                    self.active_task_name = tool_name;
                    self.execution_status = format!("Task {} of {total}", self.active_queue_index + 1);
                }
                QueueEvent::Finished { index, result, warnings, cancelled } => {
                    if let Some(entry) = self.queue.get_mut(index) {
                        entry.warnings = warnings;
                        if cancelled {
                            entry.status = QueueStatus::Cancelled;
                            entry.summary = Some("Cancelled by user".into());
                        } else {
                            match result {
                                Ok(paths) => {
                                    entry.status = QueueStatus::Completed;
                                    entry.output_count = paths.len();
                                    if !paths.is_empty() {
                                        self.last_output_dir = Some(entry.job.output_dir.clone());
                                    }
                                    entry.summary = Some(if paths.is_empty() {
                                        "Completed".into()
                                    } else {
                                        format!("{} output file(s)", paths.len())
                                    });
                                }
                                Err(error) => {
                                    entry.status = QueueStatus::Failed;
                                    entry.summary = Some(error);
                                }
                            }
                        }
                    }
                    self.queue_finished += 1;
                }
                QueueEvent::Skipped { index } => {
                    if let Some(entry) = self.queue.get_mut(index) {
                        entry.status = QueueStatus::Cancelled;
                        entry.summary = Some("Not started because the queue was stopped".into());
                    }
                    self.queue_finished += 1;
                }
                QueueEvent::QueueFinished { cancelled } => {
                    // QueueFinished is sent only after all tasks have stopped.
                    // Reap the worker instead of leaving a detached thread behind.
                    if let Some(worker) = self.queue_worker.take() {
                        let _ = worker.join();
                    }
                    self.running = false;
                    self.execution_control = None;
                    self.execution_status.clear();
                    self.active_task_name.clear();
                    self.cancel_requested = false;
                    self.logs.clear();

                    let run_entries: Vec<&QueuedJob> = self.queue_run_indices
                        .iter()
                        .filter_map(|index| self.queue.get(*index))
                        .collect();
                    let completed = run_entries.iter().filter(|entry| entry.status == QueueStatus::Completed).count();
                    let failed = run_entries.iter().filter(|entry| entry.status == QueueStatus::Failed).count();
                    let cancelled_count = run_entries.iter().filter(|entry| entry.status == QueueStatus::Cancelled).count();
                    let output_count: usize = run_entries.iter().map(|entry| entry.output_count).sum();
                    self.completed_cancelled = cancelled && cancelled_count > 0;
                    self.logs.push((true, format!(
                        "Queue finished · {completed} completed · {output_count} output(s) · {failed} failed · {cancelled_count} cancelled"
                    )));
                    for entry in &run_entries {
                        match entry.status {
                            QueueStatus::Completed => {
                                self.logs.push((true, format!("Done: {} · {} output(s)", entry.tool_name, entry.output_count)));
                            }
                            QueueStatus::Failed => {
                                self.logs.push((false, format!("{}: {}", entry.tool_name, entry.summary.as_deref().unwrap_or("Task failed"))));
                            }
                            QueueStatus::Cancelled => {
                                self.logs.push((true, format!("Cancelled: {}", entry.tool_name)));
                            }
                            _ => {}
                        }
                        for warning in &entry.warnings {
                            self.logs.push((true, format!("Warning: {warning}")));
                        }
                    }
                    if self.logs.len() > 200 {
                        self.logs.drain(0..self.logs.len() - 200);
                    }
                    self.queue_run_indices.clear();
                }
            }
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


impl Drop for ForgeApp {
    fn drop(&mut self) {
        // Closing the window must have the same cancellation semantics as Stop.
        // Keep the application process alive until the worker has reaped its
        // active child process and finished cleaning up its temporary files.
        if let Some(control) = self.execution_control.as_ref() {
            control.cancel();
        }
        if let Some(worker) = self.queue_worker.take() {
            let _ = worker.join();
        }

        self.thumbnail_control.cancel();
        for worker in self.thumbnail_workers.drain(..) {
            let _ = worker.join();
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
