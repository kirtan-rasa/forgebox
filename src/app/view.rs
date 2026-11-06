use super::ForgeApp;
use eframe::egui;

mod files;
mod icons;
mod parameters;
mod queue;
mod sidebar;
mod thumbnails;
mod widgets;
mod workspace;

pub(super) use self::files::{accepted_input_description, default_output_dir, is_supported_input_file};
pub(super) use self::icons::load_nerd_font;
pub(super) use self::thumbnails::load_thumbnail;

impl eframe::App for ForgeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_execution_status();
        self.poll_queue_events();
        self.poll_dialogs();
        self.poll_thumbnails(ctx);
        self.queue_thumbnails();
        let dropped = ctx.input(|input| input.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect::<Vec<_>>());
        if !dropped.is_empty() && !self.running {
            self.add_files(dropped);
            self.wizard_step = 0;
        }
        let dragging_external_files = ctx.input(|input| !input.raw.hovered_files.is_empty());

        let screen_width = ctx.screen_rect().width();
        let sidebar_width = if screen_width < 900.0 { 186.0 } else if screen_width < 1120.0 { 218.0 } else { 258.0 };
        // Responsive decisions must use the width that remains after the sidebar.
        // Using the full window width keeps the desktop layout active for a narrow
        // central panel, which can force the wizard controls beyond the right edge.
        let content_width = (screen_width - sidebar_width).max(0.0);
        let compact_ui = content_width < 760.0;
        self.render_sidebar(ctx, sidebar_width, compact_ui);

        self.render_workspace(ctx, content_width, compact_ui, dragging_external_files);
        if self.running || self.dialog_open || !self.thumbnail_pending.is_empty() { ctx.request_repaint_after(std::time::Duration::from_millis(100)); }
    }
}

#[cfg(test)]
mod tests {
    use super::files::reorder_item;

    #[test]
    fn reorder_item_moves_up_and_down_without_off_by_one() {
        let mut values = vec!["A", "B", "C", "D"];
        reorder_item(&mut values, 0, 3);
        assert_eq!(values, vec!["B", "C", "A", "D"]);
        reorder_item(&mut values, 2, 4);
        assert_eq!(values, vec!["B", "C", "D", "A"]);
        reorder_item(&mut values, 3, 0);
        assert_eq!(values, vec!["A", "B", "C", "D"]);
    }

    #[test]
    fn reorder_item_ignores_invalid_source() {
        let mut values = vec![1, 2, 3];
        reorder_item(&mut values, 9, 0);
        assert_eq!(values, vec![1, 2, 3]);
    }

    #[test]
    fn reorder_item_to_index_moves_item_and_keeps_stable_order() {
        let mut values = vec!["A", "B", "C", "D"];
        super::files::reorder_item_to_index(&mut values, 0, 2);
        assert_eq!(values, vec!["B", "C", "A", "D"]);
        super::files::reorder_item_to_index(&mut values, 3, 0);
        assert_eq!(values, vec!["D", "B", "C", "A"]);
        super::files::reorder_item_to_index(&mut values, 1, 1);
        assert_eq!(values, vec!["D", "B", "C", "A"]);
    }


    #[test]
    fn output_dir_prefers_downloads_when_it_exists() {
        let home = std::env::temp_dir().join(format!("forgebox-output-downloads-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(home.join("Downloads")).unwrap();
        assert_eq!(super::files::output_dir_for_home(&home), home.join("Downloads"));
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn output_dir_falls_back_to_home_when_downloads_is_missing() {
        let home = std::env::temp_dir().join(format!("forgebox-output-home-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        assert_eq!(super::files::output_dir_for_home(&home), home);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn input_file_filters_match_each_tool_category() {
        use crate::catalog::ToolId;

        assert!(super::files::is_supported_input_file(ToolId::PdfSplit, std::path::Path::new("input.PDF")));
        assert!(!super::files::is_supported_input_file(ToolId::PdfSplit, std::path::Path::new("input.png")));
        assert!(super::files::is_supported_input_file(ToolId::ImageConvert, std::path::Path::new("input.HEIC")));
        assert!(!super::files::is_supported_input_file(ToolId::ImageConvert, std::path::Path::new("input.docx")));
        assert!(super::files::is_supported_input_file(ToolId::ExtractAudio, std::path::Path::new("track.FLAC")));
        assert!(super::files::is_supported_input_file(ToolId::ExtractAudio, std::path::Path::new("clip.mkv")));
        assert!(!super::files::is_supported_input_file(ToolId::ExtractAudio, std::path::Path::new("table.xlsx")));
        assert!(super::files::is_supported_input_file(ToolId::DocToPdf, std::path::Path::new("report.docx")));
        assert!(super::files::is_supported_input_file(ToolId::DataConvert, std::path::Path::new("data.csv")));
    }

    #[test]
    fn video_extension_filter_recognizes_common_containers() {
        assert!(super::files::is_video_file(std::path::Path::new("holiday.MP4")));
        assert!(super::files::is_video_file(std::path::Path::new("clip.webm")));
        assert!(!super::files::is_video_file(std::path::Path::new("track.mp3")));
        assert!(!super::files::is_video_file(std::path::Path::new("picture.png")));
    }

    #[test]
    fn thumbnail_respects_exif_orientation() {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/exif-orientation-6.jpg");
        let image = super::thumbnails::load_thumbnail(&fixture).expect("EXIF fixture should load");
        assert!(image.size[1] > image.size[0], "EXIF orientation should display this image in portrait orientation");
    }
}
