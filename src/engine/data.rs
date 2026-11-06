use super::{
    check_cancelled, display_io, first_file, normalized_ext, os, output_for_input, report_status,
    run_ok, temp_dir, Job,
};
use csv::{ReaderBuilder, WriterBuilder};
use serde::de::{Error as _, SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use serde::{Deserializer, Serializer};
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

pub(super) fn data_convert(job: &Job, target_format: &str) -> Result<Vec<PathBuf>, String> {
    let input = first_file(job)?;
    let source_ext = input.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    let target_ext = normalized_ext(target_format);
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
    // Serialize one CSV row at a time instead of keeping the whole document in memory.
    let mut reader = ReaderBuilder::new()
        .flexible(true)
        .from_path(input)
        .map_err(|e| format!("Could not read CSV: {e}"))?;
    let headers = reader
        .headers()
        .map_err(|e| format!("Failed to read CSV headers: {e}"))?
        .clone();

    let mut unique_headers = BTreeSet::new();
    for header in &headers {
        if !unique_headers.insert(header) {
            return Err(format!("CSV contains a duplicate column name: {header}"));
        }
    }

    let result = (|| -> Result<(), String> {
        let file = fs::File::create(output).map_err(display_io)?;
        let mut serializer = serde_json::Serializer::pretty(BufWriter::new(file));
        let mut sequence = serializer
            .serialize_seq(None)
            .map_err(|error| format!("Failed to write JSON: {error}"))?;

        for (row_index, row) in reader.records().enumerate() {
            check_cancelled()?;
            if row_index == 0 || (row_index + 1) % 10_000 == 0 {
                report_status(format!("Converting CSV row {}", row_index + 1));
            }

            let record = row.map_err(|error| format!("CSV row {}: {error}", row_index + 2))?;
            if record.len() > headers.len() {
                return Err(format!(
                    "CSV row {} contains {} fields, but the header has {} columns. Extra fields would be lost; check the source CSV.",
                    row_index + 2,
                    record.len(),
                    headers.len()
                ));
            }

            let mut object = Map::new();
            for (index, header) in headers.iter().enumerate() {
                object.insert(
                    header.to_string(),
                    Value::String(record.get(index).unwrap_or("").to_string()),
                );
            }
            sequence
                .serialize_element(&object)
                .map_err(|error| format!("Failed to write JSON: {error}"))?;
        }

        sequence
            .end()
            .map_err(|error| format!("Failed to write JSON: {error}"))?;
        let mut writer = serializer.into_inner();
        writer.flush().map_err(display_io)?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(output);
    }
    result
}

pub(super) fn json_to_csv(input: &Path, output: &Path) -> Result<(), String> {
    // Discover the union of object keys in a first streaming pass. A second pass
    // writes rows, so JSON arrays with differing object keys remain supported.
    let mut header_set = BTreeSet::new();
    let mut scanned_rows = 0usize;
    let input_file = fs::File::open(input).map_err(display_io)?;
    visit_json_rows(BufReader::new(input_file), |row| {
        check_cancelled()?;
        scanned_rows += 1;
        if scanned_rows == 1 || scanned_rows % 10_000 == 0 {
            report_status(format!("Scanning JSON rows ({scanned_rows})"));
        }
        let object = row
            .as_object()
            .ok_or("Each JSON row must be an object.")?;
        header_set.extend(object.keys().cloned());
        Ok(())
    })?;

    check_cancelled()?;
    let headers: Vec<String> = header_set.into_iter().collect();
    let file = fs::File::create(output).map_err(display_io)?;
    let mut writer = WriterBuilder::new().from_writer(BufWriter::new(file));

    let result = (|| -> Result<(), String> {
        writer.write_record(&headers).map_err(|error| error.to_string())?;

        let mut written_rows = 0usize;
        let input_file = fs::File::open(input).map_err(display_io)?;
        visit_json_rows(BufReader::new(input_file), |row| {
            check_cancelled()?;
            written_rows += 1;
            if written_rows == 1 || written_rows % 10_000 == 0 {
                report_status(format!("Writing CSV row ({written_rows})"));
            }

            let object = row
                .as_object()
                .ok_or("Each JSON row must be an object.")?;
            let record = headers
                .iter()
                .map(|key| object.get(key).map(json_cell).unwrap_or_default())
                .collect::<Vec<_>>();
            writer
                .write_record(record)
                .map_err(|error| error.to_string())
        })?;

        writer.flush().map_err(display_io)
    })();

    if result.is_err() {
        drop(writer);
        let _ = fs::remove_file(output);
    }
    result
}

fn visit_json_rows<R, F>(reader: R, mut visit: F) -> Result<(), String>
where
    R: Read,
    F: FnMut(Value) -> Result<(), String>,
{
    struct RowsVisitor<'a, F> {
        visit: &'a mut F,
        row_error: &'a mut Option<String>,
    }

    impl<'de, F> Visitor<'de> for RowsVisitor<'_, F>
    where
        F: FnMut(Value) -> Result<(), String>,
    {
        type Value = ();

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("an array of JSON objects")
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            while let Some(row) = sequence.next_element::<Value>()? {
                if let Err(error) = (self.visit)(row) {
                    *self.row_error = Some(error.clone());
                    return Err(A::Error::custom(error));
                }
            }
            Ok(())
        }
    }

    let mut deserializer = serde_json::Deserializer::from_reader(reader);
    let mut row_error = None;
    let parsed = deserializer.deserialize_seq(RowsVisitor {
        visit: &mut visit,
        row_error: &mut row_error,
    });

    match parsed {
        Ok(()) => deserializer
            .end()
            .map_err(|error| format!("Invalid JSON: {error}")),
        Err(_) if row_error.is_some() => Err(row_error.expect("checked above")),
        Err(error) if error.is_data() => Err(
            "JSON must contain an array of objects, for example [{\"name\":\"Ada\"}].".into(),
        ),
        Err(error) => Err(format!("Invalid JSON: {error}")),
    }
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


#[cfg(test)]
mod tests {
    use super::{csv_to_json, json_to_csv, temp_dir};
    use csv::Reader;
    use serde_json::Value;
    use std::fs;

    #[test]
    fn csv_to_json_streams_rows_and_fills_missing_cells() {
        let directory = temp_dir("csv-json-stream-test").unwrap();
        let input = directory.join("input.csv");
        let output = directory.join("output.json");
        fs::write(&input, "name,city\nAda\nGrace,London\n").unwrap();

        csv_to_json(&input, &output).unwrap();

        let value: Value =
            serde_json::from_slice(&fs::read(&output).unwrap()).expect("output must be JSON");
        assert_eq!(
            value,
            serde_json::json!([
                {"name": "Ada", "city": ""},
                {"name": "Grace", "city": "London"}
            ])
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn json_to_csv_streams_rows_and_unions_object_keys() {
        let directory = temp_dir("json-csv-stream-test").unwrap();
        let input = directory.join("input.json");
        let output = directory.join("output.csv");
        fs::write(
            &input,
            r#"[{"z":1,"name":"Ada","nested":{"ok":true}},{"active":true,"name":null}]"#,
        )
        .unwrap();

        json_to_csv(&input, &output).unwrap();

        let mut reader = Reader::from_path(&output).unwrap();
        assert_eq!(
            reader.headers().unwrap().iter().collect::<Vec<_>>(),
            vec!["active", "name", "nested", "z"]
        );
        let rows = reader
            .records()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].get(0), Some(""));
        assert_eq!(rows[0].get(1), Some("Ada"));
        assert_eq!(rows[0].get(2), Some(r#"{"ok":true}"#));
        assert_eq!(rows[0].get(3), Some("1"));
        assert_eq!(rows[1].get(0), Some("true"));
        assert_eq!(rows[1].get(1), Some(""));
        assert_eq!(rows[1].get(2), Some(""));
        assert_eq!(rows[1].get(3), Some(""));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn json_to_csv_rejects_invalid_shapes_without_leaving_partial_output() {
        let directory = temp_dir("json-csv-invalid-test").unwrap();
        let input = directory.join("input.json");
        let output = directory.join("output.csv");

        fs::write(&input, "{}").unwrap();
        assert_eq!(
            json_to_csv(&input, &output).unwrap_err(),
            "JSON must contain an array of objects, for example [{\"name\":\"Ada\"}]."
        );
        assert!(!output.exists());

        fs::write(&input, "[1]").unwrap();
        assert_eq!(
            json_to_csv(&input, &output).unwrap_err(),
            "Each JSON row must be an object."
        );
        assert!(!output.exists());

        fs::write(&input, "[").unwrap();
        assert!(json_to_csv(&input, &output)
            .unwrap_err()
            .starts_with("Invalid JSON:"));
        assert!(!output.exists());

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn csv_to_json_removes_partial_output_on_invalid_rows() {
        let directory = temp_dir("csv-json-invalid-test").unwrap();
        let input = directory.join("input.csv");
        let output = directory.join("output.json");
        fs::write(&input, "name\nAda,extra\n").unwrap();

        assert!(csv_to_json(&input, &output)
            .unwrap_err()
            .contains("Extra fields would be lost"));
        assert!(!output.exists());

        fs::remove_dir_all(directory).unwrap();
    }
}
