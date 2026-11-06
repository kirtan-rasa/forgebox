use super::{os, run_qpdf};
use std::path::Path;

pub(super) fn pdf_page_count(input: &Path) -> Result<u32, String> {
    let output = run_qpdf(vec![os("--show-npages"), os(input)])?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let page_count = stdout.trim().parse::<u32>().map_err(|_| {
        format!("Could not read the page count from this PDF. qpdf returned: {}", stdout.trim())
    })?;
    if page_count == 0 {
        return Err("This PDF contains no pages.".into());
    }
    Ok(page_count)
}

pub(super) fn parse_pdf_page_range(value: &str, page_count: u32) -> Result<(u32, u32), String> {
    let parts: Vec<&str> = value.split(',').map(str::trim).collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err("Enter a page range as start,end. Use last for the final page, for example 1,last.".into());
    }

    let first = parts[0].parse::<u32>().ok().filter(|page| *page > 0)
        .ok_or_else(|| "The start page must be a positive number.".to_string())?;
    let end = if parts[1].eq_ignore_ascii_case("last") {
        page_count
    } else {
        parts[1].parse::<u32>().ok().filter(|page| *page > 0)
            .ok_or_else(|| "The end page must be a positive number or last.".to_string())?
    };

    if first > page_count {
        return Err(format!("Page {first} does not exist. This PDF has {page_count} pages."));
    }
    if end > page_count {
        return Err(format!("Page {end} does not exist. This PDF has {page_count} pages. Use last for the final page."));
    }
    if end < first {
        return Err("The end page must not be less than the start page.".into());
    }
    Ok((first, end))
}

pub(super) fn parse_pdf_page_order(value: &str, page_count: u32) -> Result<Vec<u32>, String> {
    if value.trim().is_empty() {
        return Err("Enter a page order, for example 3,1,2,4.".into());
    }

    value.split(',').map(str::trim).map(|part| {
        let page = part.parse::<u32>().ok().filter(|page| *page > 0)
            .ok_or_else(|| "The order must contain positive page numbers separated by commas.".to_string())?;
        if page > page_count {
            return Err(format!("Page {page} does not exist. This PDF has {page_count} pages."));
        }
        Ok(page)
    }).collect()
}
