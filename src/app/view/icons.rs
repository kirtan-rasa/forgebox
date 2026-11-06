use eframe::egui::{self, FontId};
use std::path::PathBuf;

pub(super) fn icon_font(size: f32) -> FontId {
    FontId::new(size, egui::FontFamily::Name("forgebox-icons".into()))
}


pub(super) fn collect_nerd_fonts(root: &PathBuf, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 5 { return; }
    let Ok(entries) = std::fs::read_dir(root) else { return; };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_nerd_fonts(&path, out, depth + 1);
        } else if path.file_name().and_then(|n| n.to_str()).map(|n| {
            let n = n.to_lowercase();
            n.contains("nerdfont") && (n.ends_with(".ttf") || n.ends_with(".otf"))
        }).unwrap_or(false) {
            out.push(path);
        }
    }
}

pub(super) fn install_nerd_font(ctx: &egui::Context, bytes: Option<Vec<u8>>) -> bool {
    let mut fonts = egui::FontDefinitions::default();
    let loaded = if let Some(bytes) = bytes {
        fonts.font_data.insert("forgebox-nerd".to_owned(), egui::FontData::from_owned(bytes));
        let mut icon_family = vec!["forgebox-nerd".to_owned()];
        if let Some(proportional) = fonts.families.get(&egui::FontFamily::Proportional) {
            icon_family.extend(proportional.iter().cloned());
        }
        fonts.families.insert(egui::FontFamily::Name("forgebox-icons".into()), icon_family);
        true
    } else {
        // Use readable Unicode substitutes instead of displaying missing-glyph boxes.
        let fallback = fonts.families.get(&egui::FontFamily::Proportional).cloned().unwrap_or_default();
        fonts.families.insert(egui::FontFamily::Name("forgebox-icons".into()), fallback);
        false
    };

    ctx.set_fonts(fonts);
    loaded
}

pub(in crate::app) fn load_nerd_font(ctx: &egui::Context) -> bool {
    // Fontconfig knows the active font directories. Prefer its direct answer and
    // avoid recursively scanning every system/user font folder on normal startup.
    if let Ok(output) = std::process::Command::new("fc-match")
        .args(["-f", "%{file}", "JetBrainsMono Nerd Font"])
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if path.to_lowercase().contains("nerd") {
                if let Ok(bytes) = std::fs::read(path) {
                    return install_nerd_font(ctx, Some(bytes));
                }
            }
        }
    }

    // Fallback for systems without Fontconfig or for fonts outside its database.
    let mut candidates: Vec<PathBuf> = vec![
        PathBuf::from("/usr/share/fonts/nerd-fonts/JetBrainsMonoNerdFont-Regular.ttf"),
        PathBuf::from("/usr/share/fonts/JetBrainsMonoNerdFont-Regular.ttf"),
        PathBuf::from("/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf"),
        PathBuf::from("/usr/share/fonts/truetype/nerd-fonts/JetBrainsMonoNerdFont-Regular.ttf"),
    ];
    let home = std::env::var_os("HOME");

    if let Some(home) = home {
        let home = PathBuf::from(home);
        candidates.push(home.join(".local/share/fonts/JetBrainsMonoNerdFont-Regular.ttf"));
        candidates.push(home.join(".local/share/fonts/JetBrainsMono/JetBrainsMonoNerdFont-Regular.ttf"));
        collect_nerd_fonts(&home.join(".local/share/fonts"), &mut candidates, 0);
        collect_nerd_fonts(&home.join(".fonts"), &mut candidates, 0);
    }
    for root in ["/usr/share/fonts", "/usr/local/share/fonts"] {
        collect_nerd_fonts(&PathBuf::from(root), &mut candidates, 0);
    }

    install_nerd_font(ctx, candidates.iter().find_map(|path| std::fs::read(path).ok()))
}
pub(super) fn glyph_text(icons_loaded: bool, glyph: &str) -> &str {
    if icons_loaded { return glyph; }
    match glyph {
        "󰋩" => "▧",
        "󰈙" => "▤",
        "󰈔" => "▣",
        "󰘦" | "󰓦" => "▦",
        "󰎆" => "♫",
        "󰍉" => "⌕",
        "󰉋" => "▰",
        "󰄬" => "✓",
        "󰅖" => "×",
        "󰇘" => "⇩",
        "󰩨" => "↗",
        "󰍛" | "󰍹" => "⇩",
        "󰔐" => "◐",
        "󰟖" => "◌",
        "󰋼" => "ⓘ",
        "󰕥" => "▦",
        "󰐕" => "+",
        "󰑓" => "↻",
        "󰒺" => "↕",
        "󰏘" => "◈",
        "󰐊" => "▶",
        _ => glyph,
    }
}
