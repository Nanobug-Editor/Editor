//! Table formats belong to the guest's domain cells; the host transports only bounded inert items.
use super::*;
use plugin_protocol::transfer::{ClipboardOperation, ClipboardValue, Item};
mod parse;

impl Table {
    /// Publish TSV and HTML as two representations of the same guest-owned visible grid.
    pub(super) fn copy(&mut self) -> Result<(), api::Failure> {
        if self.clipboard_request.is_some() {
            return Ok(());
        }
        self.clipboard_pasting = false;
        let result = api::guest::request(api::Operation::Editor {
            operation: api::EditorOperation::Clipboard {
                operation: ClipboardOperation::Write {
                    items: formats(
                        &self
                            .cells
                            .chunks(2)
                            .map(|row| row.to_vec())
                            .collect::<Vec<_>>(),
                    ),
                },
            },
            timeout_ms: 30_000,
        })?;
        let api::Value::Accepted(handle) = result else {
            return Err(api::Failure::new(
                api::ErrorCode::InvalidState,
                "Clipboard request was not accepted",
            ));
        };
        self.clipboard_request = Some(handle);
        self.clipboard_status = "copying".into();
        Ok(())
    }
    /// Enumerate names first; only a correlated offered HTML/TSV selection can trigger a native read.
    pub(super) fn paste(&mut self) -> Result<(), api::Failure> {
        if self.clipboard_request.is_some() {
            return Ok(());
        }
        self.clipboard_pasting = true;
        let result = api::guest::request(api::Operation::Editor {
            operation: api::EditorOperation::Clipboard {
                operation: ClipboardOperation::Enumerate,
            },
            timeout_ms: 30_000,
        })?;
        let api::Value::Accepted(handle) = result else {
            return Err(api::Failure::new(
                api::ErrorCode::InvalidState,
                "Clipboard enumeration not accepted",
            ));
        };
        self.clipboard_request = Some(handle);
        self.clipboard_status = "enumerating".into();
        Ok(())
    }
    /// Failure affects this interchange only; every selected clipboard lease has one cleanup path.
    pub(super) fn clipboard_complete(&mut self, result: Result<api::EditorValue, api::Failure>) {
        self.clipboard_request = None;
        if self.clipboard_pasting
            && self.clipboard_offer.is_none()
            && let Ok(api::EditorValue::ClipboardFormats(ClipboardValue::Offer(offer))) = &result
        {
            self.clipboard_offer = Some(offer.handle.clone());
            let read = api::guest::request(api::Operation::Editor {
                operation: api::EditorOperation::Clipboard {
                    operation: ClipboardOperation::Read {
                        offer: offer.handle.clone(),
                        preferred: vec!["text/html".into(), "text/plain;charset=utf-8".into()],
                    },
                },
                timeout_ms: 30_000,
            });
            if let Ok(api::Value::Accepted(handle)) = read {
                self.clipboard_request = Some(handle);
                self.clipboard_status = "reading".into();
                return;
            }
        }
        if let Some(handle) = self.clipboard_offer.take() {
            let _ = api::guest::request(api::Operation::CloseResource { handle });
        }
        let status = match result {
            Ok(api::EditorValue::ClipboardFormats(ClipboardValue::Written))
                if !self.clipboard_pasting =>
            {
                Ok("copied")
            }
            Ok(api::EditorValue::ClipboardFormats(ClipboardValue::Data(item)))
                if self.clipboard_pasting =>
            {
                self.import_cells(&item).map(|_| "pasted")
            }
            Err(error) => {
                self.clipboard_status = format!("{:?}", error.code);
                self.clipboard_pasting = false;
                return;
            }
            _ => Err("invalid clipboard result"),
        };
        self.clipboard_status = status.unwrap_or_else(|error| error).into();
        self.clipboard_pasting = false;
    }
    /// Validate the entire matrix before mutating any domain cell; foreign markup is never rendered.
    fn import_cells(&mut self, item: &Item) -> Result<(), &'static str> {
        self.import_at(item, self.selected)
    }
    /// Both clipboard and drops validate a complete matrix before changing the original destination.
    pub(super) fn import_at(
        &mut self,
        item: &Item,
        destination: usize,
    ) -> Result<(), &'static str> {
        item.validate().map_err(|_| "invalid clipboard data")?;
        let text = std::str::from_utf8(&item.bytes).map_err(|_| "invalid text")?;
        let rows = match item.mime.as_str() {
            "text/html" => parse::html(text)?,
            "text/plain;charset=utf-8" => parse::tsv(text)?,
            _ => return Err("unsupported table format"),
        };
        let start_row = destination / 2;
        let start_column = destination % 2;
        if rows.is_empty()
            || start_row + rows.len() > 3
            || rows
                .iter()
                .any(|row| row.is_empty() || start_column + row.len() > 2)
        {
            return Err("paste exceeds visible grid");
        }
        let mut before = Vec::new();
        for (row, values) in rows.into_iter().enumerate() {
            for (column, value) in values.into_iter().enumerate() {
                let index = (start_row + row) * 2 + start_column + column;
                before.push((index, self.cells[index].clone()));
                self.cells[index] = value;
                self.text_revisions[index] += 1;
                self.epochs[index] += 1;
                self.acknowledgements[index] = 0;
            }
        }
        self.undo.push(before);
        Ok(())
    }
}
/// Clipboard and drag export share exact ordering, quoting and inert HTML for a bounded matrix.
pub(super) fn formats(rows: &[Vec<String>]) -> Vec<Item> {
    let tsv = rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|cell| tsv_cell(cell))
                .collect::<Vec<_>>()
                .join("\t")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut html = String::from("<table>");
    for row in rows {
        html.push_str("<tr>");
        for cell in row {
            html.push_str("<td>");
            html.push_str(&escape(cell));
            html.push_str("</td>");
        }
        html.push_str("</tr>");
    }
    html.push_str("</table>");
    vec![
        Item {
            mime: "text/plain;charset=utf-8".into(),
            bytes: tsv.into_bytes(),
        },
        Item {
            mime: "text/html".into(),
            bytes: html.into_bytes(),
        },
    ]
}
/// Escape cell text as inert HTML; no source content becomes a tag, script or external resource.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
/// Delimiters inside one logical cell must never become extra rows/columns in another application.
fn tsv_cell(text: &str) -> String {
    if text.contains(['\t', '\r', '\n', '"']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.into()
    }
}
