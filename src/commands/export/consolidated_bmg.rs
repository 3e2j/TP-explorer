/*
Consolidated BMG export: combines all BMG files from multiple archives into a single JSON.

Instead of exporting multiple JSON files (text/bmgres/zel_00.json, text/bmgres1/zel_01.json, etc),
this creates a single text/messages.json containing all messages with source references.
*/

use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct BmgSource {
    pub archive: String,      // e.g., "files/res/Msgus/bmgres.arc"
    pub path: String,         // e.g., "zel_00.bmg"
    pub encoding: String,     // e.g., "shift-jis" or "latin-1"
    pub messages: Vec<Value>, // The actual message data from to_json
}

impl BmgSource {
    pub fn from_bmg(archive: String, path: String, encoding: String, bmg_json: Value) -> Self {
        let messages = if let Some(arr) = bmg_json.as_array() {
            arr.to_vec()
        } else {
            vec![]
        };

        BmgSource {
            archive,
            path,
            encoding,
            messages,
        }
    }
}

#[derive(Debug)]
pub struct ConsolidatedBmg {
    pub sources: Vec<BmgSource>,
}

impl ConsolidatedBmg {
    pub fn new() -> Self {
        ConsolidatedBmg {
            sources: Vec::new(),
        }
    }

    pub fn add_source(&mut self, source: BmgSource) {
        self.sources.push(source);
    }

    /// Convert to consolidated JSON format
    pub fn to_json(&self) -> Value {
        let sources_json: Vec<Value> = self
            .sources
            .iter()
            .map(|src| {
                json!({
                    "archive": src.archive,
                    "path": src.path,
                    "encoding": src.encoding,
                    "messages": src.messages
                })
            })
            .collect();

        json!({
            "version": 1,
            "sources": sources_json
        })
    }

    /// Convert back to individual BMG JSON formats
    /// Returns a HashMap of (archive_path, internal_path) -> (bmg_json, encoding)
    pub fn to_individual_bmgs(
        json: &Value,
    ) -> Result<HashMap<(String, String), (Value, String)>, String> {
        let mut result = HashMap::new();

        let sources = json
            .get("sources")
            .and_then(|v| v.as_array())
            .ok_or("Missing or invalid 'sources' array")?;

        for source in sources {
            let archive = source
                .get("archive")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'archive' in source")?
                .to_string();

            let path = source
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'path' in source")?
                .to_string();

            let encoding = source
                .get("encoding")
                .and_then(|v| v.as_str())
                .unwrap_or("shift-jis")
                .to_string();

            let messages = source
                .get("messages")
                .ok_or("Missing 'messages' in source")?
                .clone();

            result.insert((archive, path), (messages, encoding));
        }

        Ok(result)
    }
}
