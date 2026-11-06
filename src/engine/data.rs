use super::{display_io, first_file, normalized_ext, os, output_for_input, run_ok, temp_dir};
use csv::{ReaderBuilder, WriterBuilder};
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

pub(super) fn data_convert(job: &Job) -> Result<Vec<PathBuf>, String> {
    let input = first_file(job)?;
    let source_ext = input.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    let target_ext = normalized_ext(&job.target_format);
    if !["csv", "json", "xlsx"].contains(&source_ext.as_str()) {
        return Err("Supported input formats are CSV, JSON, and XLSX.".into());
    }
    if !["csv", "json", "xlsx"].contains(&target_ext.as_str()) {
        return Err("The output format must be CSV, JSON, or XLSX.".into());
    }
    if source_ext == target_ext {
        return Err("The input and output formats are the same.".into());
    }

    let out = output_for_input(job, input, "converted", &target_ext);
    let result = (|| -> Result<(), String> {
        match (source_ext.as_str(), target_ext.as_str()) {
            ("csv", "json") => csv_to_json(input, &out)?,
            ("json", "csv") => json_to_csv(input, &out)?,
            ("csv", "xlsx") => soffice_convert(input, "xlsx", &out)?,
            ("xlsx", "csv") => soffice_convert(input, "csv", &out)?,
            ("json", "xlsx") => {
                let temp = temp_dir("json-xlsx")?;
                let result = (|| {
                    let temp_csv = temp.join("table.csv");
                    json_to_csv(input, &temp_csv)?;
                    soffice_convert(&temp_csv, "xlsx", &out)
                })();
                let _ = fs::remove_dir_all(&temp);
                result?;
            }
            ("xlsx", "json") => {
                let temp = temp_dir("xlsx-json")?;
                let result = (|| {
                    let temp_csv = temp.join("data.csv");
                    soffice_convert(input, "csv", &temp_csv)?;
                    csv_to_json(&temp_csv, &out)
                })();
                let _ = fs::remove_dir_all(&temp);
                result?;
            }
            _ => return Err("This format combination is not supported.".into()),
        }
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&out);
        return Err(error);
    }
    Ok(vec![out])
}

pub(super) fn csv_to_json(input: &Path, output: &Path) -> Result<(), String> {
    // Keep short rows compatible by filling missing cells with empty strings,
    // but reject extra cells and duplicate headers instead of silently losing data.
    let mut reader = ReaderBuilder::new().flexible(true).from_path(input).map_err(|e| format!("Could not read CSV: {e}"))?;
    let headers = reader.headers().map_err(|e| format!("Failed to read CSV headers: {e}"))?.clone();
    let mut unique_headers = BTreeSet::new();
    for header in &headers {
        if !unique_headers.insert(header) {
            return Err(format!("CSV contains a duplicate column name: {header}"));
        }
    }

    let mut rows = Vec::new();
    for (row_index, row) in reader.records().enumerate() {
        let record = row.map_err(|e| format!("CSV row {}: {e}", row_index + 2))?;
        if record.len() > headers.len() {
            return Err(format!(
                "CSV row {} contains {} fields, but the header has {} columns. Extra fields would be lost; check the source CSV.",
                row_index + 2,
                record.len(),
                headers.len()
            ));
        }

        let mut object = Map::new();
        for (i, header) in headers.iter().enumerate() {
            object.insert(header.to_string(), Value::String(record.get(i).unwrap_or("").to_string()));
        }
        rows.push(Value::Object(object));
    }
    fs::write(output, serde_json::to_vec_pretty(&Value::Array(rows)).map_err(|e| e.to_string())?).map_err(display_io)
}

pub(super) fn json_to_csv(input: &Path, output: &Path) -> Result<(), String> {
    let text = fs::read_to_string(input).map_err(display_io)?;
    let value: Value = serde_json::from_str(&text).map_err(|e| format!("Invalid JSON: {e}"))?;
    let rows = value.as_array().ok_or("JSON must contain an array of objects, for example [{\"name\":\"Ada\"}].")?;
    let mut header_set = BTreeSet::new();
    for row in rows {
        let object = row.as_object().ok_or("Each JSON row must be an object.")?;
        header_set.extend(object.keys().cloned());
    }
    let headers: Vec<String> = header_set.into_iter().collect();
    let mut writer = WriterBuilder::new().from_path(output).map_err(|e| format!("Failed to create CSV: {e}"))?;
    writer.write_record(&headers).map_err(|e| e.to_string())?;
    for row in rows {
        let object = row.as_object().expect("validated above");
        let record = headers.iter().map(|key| object.get(key).map(json_cell).unwrap_or_default()).collect::<Vec<_>>();
        writer.write_record(record).map_err(|e| e.to_string())?;
    }
    writer.flush().map_err(display_io)
}

fn json_cell(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Number(_) | Value::Bool(_) => value.to_string(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

pub(super) fn encode_file_url_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9'
            | b'/' | b':' | b'-' | b'_' | b'.' | b'~' => encoded.push(byte as char),
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

pub(super) fn path_to_file_url(path: &Path) -> Result<String, String> {
    let absolute = fs::canonicalize(path)
        .map_err(|error| format!("Could not resolve the LibreOffice profile path: {error}"))?;
    let path_text = absolute.to_string_lossy().replace('\\', "/");
    let encoded = encode_file_url_path(&path_text);

    Ok(format!("file://{encoded}"))
}

pub(super) fn soffice_convert(input: &Path, extension: &str, output: &Path) -> Result<(), String> {
    let temp = temp_dir("office-convert")?;
    let result = (|| -> Result<(), String> {
        let profile = temp.join("profile");
        fs::create_dir_all(&profile).map_err(|error| {
            format!("Failed to create a temporary LibreOffice profile: {error}")
        })?;
        let profile_url = path_to_file_url(&profile)?;

        // LibreOffice forwards requests to an already-running instance when they
        // share the same user profile. A per-conversion profile avoids conflicts
        // with the user's desktop session and other headless conversions.
        run_ok("libreoffice", vec![
            os(format!("-env:UserInstallation={profile_url}")),
            os("--headless"),
            os("--convert-to"),
            os(extension),
            os("--outdir"),
            os(&temp),
            os(input),
        ])?;
        let generated = temp.join(format!("{}.{}", input.file_stem().and_then(|s| s.to_str()).unwrap_or("output"), extension));
        if !generated.exists() {
            return Err(format!("LibreOffice did not create the expected file {}. Check whether the input format is supported.", generated.display()));
        }
        fs::copy(&generated, output).map_err(|e| format!("Failed to save the output: {e}"))?;
        Ok(())
    })();
    let _ = fs::remove_dir_all(&temp);
    if result.is_err() { let _ = fs::remove_file(output); }
    result
}
