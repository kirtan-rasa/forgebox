#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    Images,
    Pdf,
    Documents,
    Data,
    Media,
}

impl Category {
    pub const ALL: [Category; 5] = [
        Category::Images,
        Category::Pdf,
        Category::Documents,
        Category::Data,
        Category::Media,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Category::Images => "Images",
            Category::Pdf => "PDF",
            Category::Documents => "Documents",
            Category::Data => "Data",
            Category::Media => "Audio & Video",
        }
    }

    pub fn glyph(self) -> &'static str {
        match self {
            Category::Images => "󰋩",
            Category::Pdf => "󰈙",
            Category::Documents => "󰈔",
            Category::Data => "󰘦",
            Category::Media => "󰎆",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToolId {
    ImageConvert,
    ImagesToPdf,
    ImageResize,
    ImageCompress,
    ImageGrayscale,
    RemoveBackground,
    ExtractExif,
    ImageOcr,
    GenerateQr,
    LongPngToPdf,
    PdfMerge,
    PdfSplit,
    PdfCompress,
    PdfRotate,
    PdfReorder,
    PdfToImages,
    PdfWatermark,
    DocToPdf,
    PdfToDoc,
    DataConvert,
    ExtractAudio,
    VideoConvert,
    VideoToGif,
}

pub struct ToolInfo {
    pub id: ToolId,
    pub category: Category,
    pub name: &'static str,
    pub subtitle: &'static str,
    pub description: &'static str,
    pub glyph: &'static str,
    pub dependencies: &'static str,
}

pub const TOOLS: [ToolInfo; 23] = [
    ToolInfo { id: ToolId::ImagesToPdf, category: Category::Images, name: "Images to PDF", subtitle: "Multiple images in one multi-page PDF", description: "Combine selected images into a single PDF. Each image becomes a separate page in list order.", glyph: "󰈙", dependencies: "ImageMagick + img2pdf" },
    ToolInfo { id: ToolId::ImageConvert, category: Category::Images, name: "Image Converter", subtitle: "PNG · JPG · WebP · TIFF · HEIC", description: "Convert images between popular formats. Select multiple files at once.", glyph: "󰓦", dependencies: "ImageMagick (magick or convert)" },
    ToolInfo { id: ToolId::ImageResize, category: Category::Images, name: "Resize Image", subtitle: "Set width and height without distortion", description: "Resize an image to fit the requested dimensions while preserving its aspect ratio.", glyph: "󰩨", dependencies: "ImageMagick" },
    ToolInfo { id: ToolId::ImageCompress, category: Category::Images, name: "Compress Image", subtitle: "Smaller files, originals preserved", description: "Adjust quality for JPEG, WebP, and supported ImageMagick formats.", glyph: "󰍛", dependencies: "ImageMagick" },
    ToolInfo { id: ToolId::ImageGrayscale, category: Category::Images, name: "Grayscale", subtitle: "Black and white", description: "Convert an image to grayscale while preserving its format.", glyph: "󰔐", dependencies: "ImageMagick" },
    ToolInfo { id: ToolId::RemoveBackground, category: Category::Images, name: "Remove Background", subtitle: "Transparent PNG", description: "Remove an image background locally using an AI model. The first run may download the model separately.", glyph: "󰟖", dependencies: "rembg (optional Python setup)" },
    ToolInfo { id: ToolId::ExtractExif, category: Category::Images, name: "EXIF Metadata", subtitle: "Inspect image properties", description: "Export image metadata to a structured JSON file.", glyph: "󰋼", dependencies: "ExifTool" },
    ToolInfo { id: ToolId::ImageOcr, category: Category::Images, name: "Extract Text (OCR)", subtitle: "Image to TXT", description: "Extract text from a screenshot, document photo, or scan.", glyph: "T", dependencies: "Tesseract OCR + language data" },
    ToolInfo { id: ToolId::GenerateQr, category: Category::Images, name: "QR Code Generator", subtitle: "A URL or any text", description: "Create a QR code and save it as a PNG.", glyph: "󰕥", dependencies: "Built in" },
    ToolInfo { id: ToolId::LongPngToPdf, category: Category::Images, name: "Long Image to PDF", subtitle: "Split a tall screenshot into pages", description: "Split a very tall image across multiple pages and combine them into a PDF.", glyph: "󰈙", dependencies: "ImageMagick + img2pdf" },
    ToolInfo { id: ToolId::PdfMerge, category: Category::Pdf, name: "Merge PDFs", subtitle: "Multiple files → one", description: "Merge PDFs in the selected order. The file list order is preserved.", glyph: "󰐕", dependencies: "Poppler (pdfunite)" },
    ToolInfo { id: ToolId::PdfSplit, category: Category::Pdf, name: "Split PDF", subtitle: "Export a page range as separate files", description: "Save each page in a selected range as its own PDF file. Use last to include the final page.", glyph: "󰈙", dependencies: "Poppler (pdfseparate) + qpdf" },
    ToolInfo { id: ToolId::PdfCompress, category: Category::Pdf, name: "Compress PDF", subtitle: "Balance file size and quality", description: "Reduce PDF size with Ghostscript. Choose a profile for screen, e-book, or print use.", glyph: "󰍹", dependencies: "Ghostscript (gs)" },
    ToolInfo { id: ToolId::PdfRotate, category: Category::Pdf, name: "Rotate PDF", subtitle: "90° · 180° · 270°", description: "Rotate PDF pages without re-encoding their contents.", glyph: "󰑓", dependencies: "qpdf" },
    ToolInfo { id: ToolId::PdfReorder, category: Category::Pdf, name: "Reorder PDF Pages", subtitle: "For example: 3,1,2,4", description: "Rebuild a document from the listed page numbers. Pages can be repeated; pages not listed are omitted.", glyph: "󰒺", dependencies: "qpdf" },
    ToolInfo { id: ToolId::PdfToImages, category: Category::Pdf, name: "PDF to Images", subtitle: "PNG or JPG, one file per page", description: "Render each PDF page as a separate image at the selected DPI.", glyph: "󰋩", dependencies: "Poppler (pdftoppm)" },
    ToolInfo { id: ToolId::PdfWatermark, category: Category::Pdf, name: "PDF Watermark", subtitle: "Overlay one PDF onto another", description: "Overlay the first page of a second PDF on every page of the main document.", glyph: "󰏘", dependencies: "pdftk" },
    ToolInfo { id: ToolId::DocToPdf, category: Category::Documents, name: "DOCX to PDF", subtitle: "Office documents to PDF", description: "Convert DOC/DOCX files to PDF using an installed office suite.", glyph: "D→P", dependencies: "LibreOffice" },
    ToolInfo { id: ToolId::PdfToDoc, category: Category::Documents, name: "PDF to DOCX", subtitle: "Extract text into an editable document", description: "Extract selectable PDF text and create a DOCX. Page layout and images are not preserved; scanned PDFs need OCR first.", glyph: "P→D", dependencies: "Poppler (pdftotext) + LibreOffice" },
    ToolInfo { id: ToolId::DataConvert, category: Category::Data, name: "CSV · JSON · Excel", subtitle: "Convert tables and data", description: "Convert CSV, JSON, and XLSX files. Text formats use built-in conversion; Excel operations require LibreOffice.", glyph: "󰓦", dependencies: "LibreOffice for XLSX" },
    ToolInfo { id: ToolId::ExtractAudio, category: Category::Media, name: "Extract Audio", subtitle: "Video to MP3 / WAV / FLAC and more", description: "Extract an audio track from a video without re-encoding the video stream.", glyph: "󰎆", dependencies: "FFmpeg" },
    ToolInfo { id: ToolId::VideoConvert, category: Category::Media, name: "Video Converter", subtitle: "MP4 · WebM · MKV · MOV", description: "Convert videos to a convenient format with quality and encoding-speed controls.", glyph: "󰐊", dependencies: "FFmpeg with support for the selected codec" },
    ToolInfo { id: ToolId::VideoToGif, category: Category::Media, name: "Video to GIF", subtitle: "Create an animated GIF from a clip", description: "Create a GIF with adjustable frame rate and width. Best for short clips.", glyph: "󰐊", dependencies: "FFmpeg" },
];

pub fn info(id: ToolId) -> &'static ToolInfo {
    TOOLS.iter().find(|tool| tool.id == id).expect("every ToolId must be catalogued")
}
