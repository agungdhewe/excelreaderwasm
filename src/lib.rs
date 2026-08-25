use std::io::Cursor;
use calamine::{open_workbook_auto_from_rs, DataType, Reader};
use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use crc32fast::Hasher as CrcHasher;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChunkManifestItem {
    pub chunk_index: usize,
    pub chunk_size: usize,
    pub start_row: usize,
    pub end_row: usize,
    pub checksum: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChunkMeta {
    pub chunk_index: usize,
    pub total_chunks: usize,
    pub chunk_size: usize,
    pub start_row: usize,
    pub end_row: usize,
    pub total_rows: usize,
    pub is_last_chunk: bool,
    pub progress_percent: f64,
    pub checksum: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProcessSummary {
    pub success: bool,
    pub total_rows: usize,
    pub total_chunks: usize,
    pub chunk_size: usize,
    pub sheet_name: String,
    pub headers: Vec<String>,
    pub total_checksum: String,
    pub chunk_manifest: Vec<ChunkManifestItem>,
}

fn cell_to_json_value(cell: &calamine::Data) -> JsonValue {
    match cell {
        calamine::Data::Empty => JsonValue::Null,
        calamine::Data::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                JsonValue::Null
            } else {
                JsonValue::String(trimmed.to_string())
            }
        }
        calamine::Data::Int(i) => JsonValue::Number((*i).into()),
        calamine::Data::Float(f) => {
            if f.fract() == 0.0 && *f >= (i64::MIN as f64) && *f <= (i64::MAX as f64) {
                JsonValue::Number((*f as i64).into())
            } else if let Some(n) = serde_json::Number::from_f64(*f) {
                JsonValue::Number(n)
            } else {
                JsonValue::Null
            }
        }
        calamine::Data::Bool(b) => JsonValue::Bool(*b),
        calamine::Data::DateTime(dt) => {
            if let Some(d) = dt.as_datetime() {
                if d.time() == chrono::NaiveTime::MIN {
                    JsonValue::String(d.format("%Y-%m-%d").to_string())
                } else {
                    JsonValue::String(d.format("%Y-%m-%d %H:%M:%S").to_string())
                }
            } else if let Some(n) = serde_json::Number::from_f64(dt.as_f64()) {
                JsonValue::Number(n)
            } else {
                JsonValue::Null
            }
        }
        calamine::Data::DateTimeIso(s) => JsonValue::String(s.clone()),
        calamine::Data::DurationIso(s) => JsonValue::String(s.clone()),
        calamine::Data::Error(e) => JsonValue::String(format!("#ERROR: {:?}", e)),
    }
}

/// Normalizes header names for flexible comparison (trimmed, lowercased).
fn normalize_header(h: &str) -> String {
    h.trim().to_lowercase()
}

/// Parses the valid_header parameter.
pub fn parse_valid_headers(valid_header: &str) -> Vec<String> {
    let trimmed = valid_header.trim();
    if trimmed.is_empty() {
        return vec![];
    }

    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        if let Ok(vec) = serde_json::from_str::<Vec<String>>(trimmed) {
            return vec.into_iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        }
    }
    
    if trimmed.contains('|') {
        trimmed.split('|').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    } else if trimmed.contains(',') {
        trimmed.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    } else if trimmed.contains(';') {
        trimmed.split(';').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    } else if trimmed.contains('\t') {
        trimmed.split('\t').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    } else {
        vec![trimmed.to_string()]
    }
}

/// Parses mapping_header parameter.
pub fn parse_mapping_headers(mapping_str: &str) -> Result<Vec<(String, String)>, String> {
    let trimmed = mapping_str.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    if trimmed.starts_with('{') {
        let val: JsonValue = serde_json::from_str(trimmed)
            .map_err(|e| format!("Format JSON mappingHeader tidak valid: {}", e))?;
        
        if let JsonValue::Object(map) = val {
            let mut list = Vec::new();
            for (k, v) in map {
                if let Some(target_col) = v.as_str() {
                    list.push((k, target_col.trim().to_string()));
                } else {
                    list.push((k, v.to_string()));
                }
            }
            Ok(list)
        } else {
            Err("mappingHeader harus berupa JSON Object".to_string())
        }
    } else {
        let mut list = Vec::new();
        for pair in trimmed.split(',') {
            let parts: Vec<&str> = pair.split(':').map(|s| s.trim()).collect();
            if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
                list.push((parts[0].to_string(), parts[1].to_string()));
            }
        }
        Ok(list)
    }
}

fn compute_crc32(bytes: &[u8]) -> String {
    let mut hasher = CrcHasher::new();
    hasher.update(bytes);
    format!("{:08x}", hasher.finalize())
}

#[wasm_bindgen]
pub struct ExcelReader {
    sheet_names: Vec<String>,
    bytes: Vec<u8>,
}

#[wasm_bindgen]
impl ExcelReader {
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: &[u8]) -> Result<ExcelReader, JsValue> {
        let cursor = Cursor::new(bytes.to_vec());
        let workbook = open_workbook_auto_from_rs(cursor)
            .map_err(|e| JsValue::from_str(&format!("Gagal membuka file spreadsheet: {}", e)))?;
        
        let sheet_names = workbook.sheet_names().to_vec();
        if sheet_names.is_empty() {
            return Err(JsValue::from_str("Spreadsheet tidak memiliki worksheet"));
        }
        
        Ok(ExcelReader {
            sheet_names,
            bytes: bytes.to_vec(),
        })
    }

    #[wasm_bindgen(js_name = getSheetNames)]
    pub fn get_sheet_names(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen_to_js(&self.sheet_names)
    }

    /// Retrieve headers for a given sheet index
    #[wasm_bindgen(js_name = getHeaders)]
    pub fn get_headers(&self, sheet_index: Option<usize>) -> Result<JsValue, JsValue> {
        let cursor = Cursor::new(&self.bytes);
        let mut workbook = open_workbook_auto_from_rs(cursor)
            .map_err(|e| JsValue::from_str(&format!("Gagal membaca workbook: {}", e)))?;

        let sheet_idx = sheet_index.unwrap_or(0);
        let sheet_name = self.sheet_names.get(sheet_idx)
            .ok_or_else(|| JsValue::from_str(&format!("Sheet index {} tidak ditemukan", sheet_idx)))?;

        let range = workbook.worksheet_range(sheet_name)
            .map_err(|e| JsValue::from_str(&format!("Gagal membaca sheet '{}': {}", sheet_name, e)))?;

        let mut row_iter = range.rows();
        let header_row = row_iter.next()
            .ok_or_else(|| JsValue::from_str("File spreadsheet kosong (tidak ada baris header)"))?;

        let actual_headers: Vec<String> = header_row.iter().map(|c| c.to_string().trim().to_string()).collect();
        serde_wasm_bindgen_to_js(&actual_headers)
    }

    /// Validates the sheet header against valid_header specification
    #[wasm_bindgen(js_name = validateHeaders)]
    pub fn validate_headers(&self, valid_header: &str, sheet_index: Option<usize>) -> Result<JsValue, JsValue> {
        let cursor = Cursor::new(&self.bytes);
        let mut workbook = open_workbook_auto_from_rs(cursor)
            .map_err(|e| JsValue::from_str(&format!("Gagal membaca workbook: {}", e)))?;

        let sheet_idx = sheet_index.unwrap_or(0);
        let sheet_name = self.sheet_names.get(sheet_idx)
            .ok_or_else(|| JsValue::from_str(&format!("Sheet index {} tidak ditemukan", sheet_idx)))?;

        let range = workbook.worksheet_range(sheet_name)
            .map_err(|e| JsValue::from_str(&format!("Gagal membaca sheet '{}': {}", sheet_name, e)))?;

        let mut row_iter = range.rows();
        let header_row = row_iter.next()
            .ok_or_else(|| JsValue::from_str("File spreadsheet kosong (tidak ada baris header)"))?;

        let actual_headers: Vec<String> = header_row.iter().map(|c| c.to_string().trim().to_string()).collect();
        let expected_headers = parse_valid_headers(valid_header);

        if !expected_headers.is_empty() {
            let actual_norm: Vec<String> = actual_headers.iter().map(|h| normalize_header(h)).collect();
            let mut missing = Vec::new();

            for exp in &expected_headers {
                let exp_norm = normalize_header(exp);
                if !actual_norm.iter().any(|act| act == &exp_norm) {
                    missing.push(exp.clone());
                }
            }

            if !missing.is_empty() {
                return Err(JsValue::from_str(&format!(
                    "Validasi header gagal!\nHeader yang diharapkan: {}\nHeader yang ditemukan: {}\nKolom yang hilang: {}",
                    expected_headers.join(" | "),
                    actual_headers.join(" | "),
                    missing.join(", ")
                )));
            }
        }

        let res = js_sys::Object::new();
        js_sys::Reflect::set(&res, &JsValue::from_str("valid"), &JsValue::from_bool(true))?;
        let js_headers = serde_wasm_bindgen_to_js(&actual_headers)?;
        js_sys::Reflect::set(&res, &JsValue::from_str("headers"), &js_headers)?;
        Ok(res.into())
    }

    /// Process spreadsheet in chunks and either trigger callback or return all chunks.
    #[wasm_bindgen(js_name = parseSpreadsheet)]
    pub fn parse_spreadsheet(
        &self,
        valid_header: &str,
        mapping_header: &str,
        row_chunk: usize,
        sheet_index: Option<usize>,
        callback: Option<js_sys::Function>,
    ) -> Result<JsValue, JsValue> {
        let cursor = Cursor::new(&self.bytes);
        let mut workbook = open_workbook_auto_from_rs(cursor)
            .map_err(|e| JsValue::from_str(&format!("Gagal membaca workbook: {}", e)))?;

        let sheet_idx = sheet_index.unwrap_or(0);
        let sheet_name = self.sheet_names.get(sheet_idx)
            .ok_or_else(|| JsValue::from_str(&format!("Sheet index {} tidak ditemukan", sheet_idx)))?
            .clone();

        let range = workbook.worksheet_range(&sheet_name)
            .map_err(|e| JsValue::from_str(&format!("Gagal membaca sheet '{}': {}", sheet_name, e)))?;

        let mut row_iter = range.rows();
        
        // 1. Read Header Row (Row 0)
        let header_row = row_iter.next()
            .ok_or_else(|| JsValue::from_str("File spreadsheet kosong (tidak ada baris header)"))?;

        let actual_headers: Vec<String> = header_row.iter().map(|c| c.to_string().trim().to_string()).collect();

        // 2. Validate Headers if valid_header is provided
        let expected_headers = parse_valid_headers(valid_header);
        if !expected_headers.is_empty() {
            let actual_norm: Vec<String> = actual_headers.iter().map(|h| normalize_header(h)).collect();
            let mut missing_headers = Vec::new();

            for exp in &expected_headers {
                let exp_norm = normalize_header(exp);
                if !actual_norm.iter().any(|act| act == &exp_norm) {
                    missing_headers.push(exp.clone());
                }
            }

            if !missing_headers.is_empty() {
                let err_msg = format!(
                    "Validasi header gagal!\nHeader yang diharapkan: {}\nHeader yang ditemukan di file: {}\nKolom yang tidak ditemukan: {}",
                    expected_headers.join(" | "),
                    actual_headers.join(" | "),
                    missing_headers.join(", ")
                );
                return Err(JsValue::from_str(&err_msg));
            }
        }

        // 3. Prepare Mapping
        let mapping_pairs = parse_mapping_headers(mapping_header)
            .map_err(|e| JsValue::from_str(&e))?;

        // Map: Output Field Name -> Column Index in Sheet
        let mut field_to_col_idx: Vec<(String, usize)> = Vec::new();
        if !mapping_pairs.is_empty() {
            for (json_field, excel_col_name) in mapping_pairs {
                let norm_excel_col = normalize_header(&excel_col_name);
                if let Some(col_idx) = actual_headers.iter().position(|h| normalize_header(h) == norm_excel_col) {
                    field_to_col_idx.push((json_field, col_idx));
                } else {
                    return Err(JsValue::from_str(&format!(
                        "Kolom '{}' pada mappingHeader tidak ditemukan di file Excel (Kolom tersedia: {})",
                        excel_col_name,
                        actual_headers.join(", ")
                    )));
                }
            }
        } else {
            // If no mapping provided, use actual headers as json keys
            for (idx, h) in actual_headers.iter().enumerate() {
                if !h.is_empty() {
                    field_to_col_idx.push((h.clone(), idx));
                }
            }
        }

        // 4. Read Data Rows in Chunks
        let chunk_size = if row_chunk == 0 { 1000 } else { row_chunk };

        // Collect all non-empty data rows
        let mut data_rows: Vec<&[calamine::Data]> = Vec::new();
        for row in row_iter {
            let has_content = row.iter().any(|c| !c.is_empty());
            if has_content {
                data_rows.push(row);
            }
        }
        let total_rows = data_rows.len();
        let total_chunks = if total_rows == 0 { 0 } else { (total_rows + chunk_size - 1) / chunk_size };

        let mut collected_chunks: Vec<serde_json::Value> = Vec::new();
        let mut manifest: Vec<ChunkManifestItem> = Vec::with_capacity(total_chunks);
        let mut overall_hasher = CrcHasher::new();

        for chunk_idx in 0..total_chunks {
            let start = chunk_idx * chunk_size;
            let end = std::cmp::min(start + chunk_size, total_rows);
            let slice = &data_rows[start..end];

            let mut chunk_items = Vec::with_capacity(slice.len());
            for row in slice {
                let mut map = serde_json::Map::new();
                for (field_name, col_idx) in &field_to_col_idx {
                    let val = if let Some(cell) = row.get(*col_idx) {
                        cell_to_json_value(cell)
                    } else {
                        JsonValue::Null
                    };
                    map.insert(field_name.clone(), val);
                }
                chunk_items.push(JsonValue::Object(map));
            }

            let chunk_json = serde_json::to_string(&chunk_items)
                .map_err(|e| JsValue::from_str(&format!("JSON serialize error: {}", e)))?;
            
            let chunk_checksum = compute_crc32(chunk_json.as_bytes());
            overall_hasher.update(chunk_json.as_bytes());

            manifest.push(ChunkManifestItem {
                chunk_index: chunk_idx + 1,
                chunk_size: slice.len(),
                start_row: start + 1,
                end_row: end,
                checksum: chunk_checksum.clone(),
            });

            let meta = ChunkMeta {
                chunk_index: chunk_idx + 1,
                total_chunks,
                chunk_size: slice.len(),
                start_row: start + 1,
                end_row: end,
                total_rows,
                is_last_chunk: chunk_idx + 1 == total_chunks,
                progress_percent: if total_rows == 0 { 100.0 } else { ((end as f64) / (total_rows as f64) * 100.0).min(100.0) },
                checksum: chunk_checksum,
            };

            let meta_json = serde_json::to_string(&meta)
                .map_err(|e| JsValue::from_str(&format!("JSON meta serialize error: {}", e)))?;

            if let Some(ref cb) = callback {
                let js_data = js_sys::JSON::parse(&chunk_json)?;
                let js_meta = js_sys::JSON::parse(&meta_json)?;
                cb.call2(&JsValue::NULL, &js_data, &js_meta)?;
            } else {
                collected_chunks.push(JsonValue::Array(chunk_items));
            }
        }

        let total_checksum = format!("{:08x}", overall_hasher.finalize());

        let summary = ProcessSummary {
            success: true,
            total_rows,
            total_chunks,
            chunk_size,
            sheet_name,
            headers: actual_headers,
            total_checksum,
            chunk_manifest: manifest,
        };

        let summary_json = serde_json::to_string(&summary)
            .map_err(|e| JsValue::from_str(&format!("Summary serialize error: {}", e)))?;

        let res_obj = js_sys::Object::new();
        let js_summary = js_sys::JSON::parse(&summary_json)?;
        js_sys::Reflect::set(&res_obj, &JsValue::from_str("summary"), &js_summary)?;

        if callback.is_none() {
            let chunks_json = serde_json::to_string(&collected_chunks)
                .map_err(|e| JsValue::from_str(&format!("Chunks serialize error: {}", e)))?;
            let js_chunks = js_sys::JSON::parse(&chunks_json)?;
            js_sys::Reflect::set(&res_obj, &JsValue::from_str("chunks"), &js_chunks)?;
        }

        Ok(res_obj.into())
    }
}

/// Standalone helper to parse spreadsheet directly from bytes
#[wasm_bindgen(js_name = parseSpreadsheetDirect)]
pub fn parse_spreadsheet_direct(
    bytes: &[u8],
    valid_header: &str,
    mapping_header: &str,
    row_chunk: usize,
    sheet_index: Option<usize>,
    callback: Option<js_sys::Function>,
) -> Result<JsValue, JsValue> {
    let reader = ExcelReader::new(bytes)?;
    reader.parse_spreadsheet(valid_header, mapping_header, row_chunk, sheet_index, callback)
}

fn serde_wasm_bindgen_to_js<T: Serialize>(val: &T) -> Result<JsValue, JsValue> {
    let json_str = serde_json::to_string(val)
        .map_err(|e| JsValue::from_str(&format!("Serialize error: {}", e)))?;
    js_sys::JSON::parse(&json_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_headers() {
        let h1 = parse_valid_headers("No|Nama|Alamat|Kota");
        assert_eq!(h1, vec!["No", "Nama", "Alamat", "Kota"]);

        let h2 = parse_valid_headers("No, Nama, Alamat, Kota");
        assert_eq!(h2, vec!["No", "Nama", "Alamat", "Kota"]);

        let h3 = parse_valid_headers("[\"No\", \"Nama\", \"Alamat\", \"Kota\"]");
        assert_eq!(h3, vec!["No", "Nama", "Alamat", "Kota"]);
    }

    #[test]
    fn test_parse_mapping_headers() {
        let m1 = parse_mapping_headers("{\"no\":\"No\", \"alamat\":\"Alamat\"}").unwrap();
        assert_eq!(m1, vec![("no".to_string(), "No".to_string()), ("alamat".to_string(), "Alamat".to_string())]);

        let m2 = parse_mapping_headers("no:No, alamat:Alamat").unwrap();
        assert_eq!(m2, vec![("no".to_string(), "No".to_string()), ("alamat".to_string(), "Alamat".to_string())]);
    }
}
