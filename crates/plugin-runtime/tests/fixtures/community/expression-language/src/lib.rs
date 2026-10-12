//! An independent expression language implements three readonly experimental callbacks without LSP.
//! Its business edits are ordinary versioned plans; the host owns all presentation and effects.
use plugin_protocol::{
    api,
    bindings::{Guest, export},
    diagnostics, direct, ui, workspace,
};

/// Translate plugin-owned UTF-8 token offsets into the public strict UTF-16 location contract.
fn position(text: &str, offset: usize) -> api::TextPosition {
    let prefix = &text[..offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
    let tail = prefix.rsplit('\n').next().unwrap();
    api::TextPosition {
        line,
        character: tail.encode_utf16().count() as u32,
    }
}

/// A token includes Unicode letters; adjacent punctuation never becomes a navigation target.
fn token(text: &str, cursor: usize) -> Option<(usize, usize)> {
    let start = text[..cursor]
        .char_indices()
        .rev()
        .take_while(|(_, ch)| ch.is_alphanumeric() || *ch == '_')
        .last()
        .map_or(cursor, |(index, _)| index);
    let end = text[cursor..]
        .char_indices()
        .take_while(|(_, ch)| ch.is_alphanumeric() || *ch == '_')
        .last()
        .map_or(cursor, |(index, ch)| cursor + index + ch.len_utf8());
    (start < end).then_some((start, end))
}

/// Return immutable policy results for one exact source and request; no callback calls host APIs.
fn feature(request: &direct::Request) -> direct::Result {
    let text = &request.source.text;
    match request.feature {
        direct::Feature::References => {
            let locations = token(text, request.cursor)
                .map(|(start, end)| {
                    let name = &text[start..end];
                    text.match_indices(name)
                        .filter(|(start, _)| {
                            text[..*start]
                                .chars()
                                .next_back()
                                .is_none_or(|ch| !ch.is_alphanumeric() && ch != '_')
                                && text[*start + name.len()..]
                                    .chars()
                                    .next()
                                    .is_none_or(|ch| !ch.is_alphanumeric() && ch != '_')
                        })
                        .take(256)
                        .map(|(offset, _)| direct::Location {
                            resource: api::ResourceIdentity::Local {
                                path: request.source.document.path.clone(),
                            },
                            position: position(text, offset),
                            label: Some(name.to_owned()),
                        })
                        .collect()
                })
                .unwrap_or_default();
            direct::Result::References { locations }
        }
        direct::Feature::CodeActions => {
            let actions = text
                .match_indices("add(")
                .next()
                .map(|(start, _)| diagnostics::Fix {
                    title: "Use sum / 使用 sum".into(),
                    plan: workspace::Plan {
                        title: "Replace expression function / 替换表达式函数".into(),
                        origin: Some(request.source.document.clone()),
                        changes: vec![workspace::Change::EditDocument {
                            document: request.source.document.clone(),
                            edits: vec![api::TextEdit {
                                range: api::TextRange {
                                    start,
                                    end: start + 3,
                                },
                                text: "sum".into(),
                            }],
                            selection: api::TextRange {
                                start: start + 3,
                                end: start + 3,
                            },
                        }],
                    },
                })
                .into_iter()
                .collect();
            direct::Result::CodeActions { actions }
        }
        direct::Feature::SignatureHelp => {
            let prefix = &text[..request.cursor];
            let help = prefix
                .rfind('(')
                .filter(|start| !prefix[*start..].contains(')'))
                .map(|start| {
                    let parameter = prefix[start + 1..]
                        .bytes()
                        .filter(|byte| *byte == b',')
                        .count()
                        .min(1) as u32;
                    direct::SignatureHelp {
                        signatures: vec![direct::Signature {
                            label: "sum(left, right)".into(),
                            documentation: Some(
                                "Add two expression values / 相加两个表达式值".into(),
                            ),
                            parameters: vec![
                                direct::Parameter {
                                    label: "left".into(),
                                    documentation: None,
                                },
                                direct::Parameter {
                                    label: "right".into(),
                                    documentation: None,
                                },
                            ],
                            active_parameter: Some(parameter),
                        }],
                        active_signature: Some(0),
                        active_parameter: Some(parameter),
                    }
                });
            direct::Result::SignatureHelp { help }
        }
    }
}

/// Stateless expression policy receives immutable source in each isolated pure request.
struct Expressions;
impl Guest for Expressions {
    /// Pure callbacks emit one correlated result; lifecycle callbacks supply a native usage surface.
    fn dispatch(payload: String) -> Result<String, String> {
        api::guest::dispatch(&payload, |input| {
            // Pure source data and results are transient; a fresh instance needs no process memory.
            if matches!(input, api::Input::Snapshot) {
                return Ok(api::Output {
                    snapshot: Some(Default::default()),
                    ..Default::default()
                });
            }
            if let api::Input::Event {
                event: api::Notification::LanguageDirect(request),
                ..
            } = input
            {
                if !request.valid() {
                    return Err(api::Failure::new(
                        api::ErrorCode::InvalidRequest,
                        "Invalid expression source",
                    ));
                }
                let result = feature(&request);
                return Ok(api::Output {
                    language_direct: Some(direct::Proposal {
                        request: request.request,
                        document: request.source.document,
                        result,
                    }),
                    ..Default::default()
                });
            }
            if !matches!(input, api::Input::Activate) {
                return Ok(Default::default());
            }
            Ok(api::Output {
                views: vec![api::View {
                    panel: "language".into(),
                    document: ui::Document::new(ui::Node::text(
                        "language-usage",
                        "Expressions / 表达式: shift-F12 references · ctrl-. fixes · ctrl-shift-space parameters",
                    )),
                }],
                ..Default::default()
            })
        })
    }
}
export!(Expressions);
