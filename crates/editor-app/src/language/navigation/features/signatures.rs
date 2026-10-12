//! Standard signatures become bounded plain cards with strict UTF-16 label spans.
use super::*;

/// The server owns overload choice; invalid indices follow standard LSP defaults instead of
/// selecting an unrelated parameter. Documentation is plain display text, never executable markup.
pub(super) fn decode(response: Value) -> anyhow::Result<direct::Result> {
    if response.is_null() {
        return Ok(direct::Result::SignatureHelp { help: None });
    }
    let help: lsp_types::SignatureHelp =
        serde_json::from_value(response).context("Decode LSP signatures")?;
    if help.signatures.is_empty() {
        return Ok(direct::Result::SignatureHelp { help: None });
    }
    if help.signatures.len() > 16 {
        return Err(Failure::new(ErrorCode::LimitExceeded, "Too many signature overloads").into());
    }
    let signatures = help
        .signatures
        .into_iter()
        .map(signature)
        .collect::<anyhow::Result<Vec<_>>>()?;
    let active_signature = help
        .active_signature
        .filter(|index| (*index as usize) < signatures.len())
        .unwrap_or(0);
    let active_parameter = parameter_index(
        help.active_parameter.or(Some(0)),
        signatures[active_signature as usize].parameters.len(),
    );
    Ok(direct::Result::SignatureHelp {
        help: Some(direct::SignatureHelp {
            signatures,
            active_signature: Some(active_signature),
            active_parameter,
        }),
    })
}

/// Parameter label offsets count UTF-16 units within a full signature, rather than byte columns.
fn signature(signature: lsp_types::SignatureInformation) -> anyhow::Result<direct::Signature> {
    let parameters = signature.parameters.unwrap_or_default();
    if parameters.len() > 32 {
        return Err(Failure::new(ErrorCode::LimitExceeded, "Too many signature parameters").into());
    }
    let parameters = parameters
        .into_iter()
        .map(|parameter| {
            let label = match parameter.label {
                lsp_types::ParameterLabel::Simple(label) if signature.label.contains(&label) => {
                    label
                }
                lsp_types::ParameterLabel::Simple(_) => {
                    return Err(Failure::new(
                        ErrorCode::InvalidRequest,
                        "Parameter label is outside signature",
                    )
                    .into());
                }
                lsp_types::ParameterLabel::LabelOffsets([start, end]) => {
                    let start = label_offset(&signature.label, start)?;
                    let end = label_offset(&signature.label, end)?;
                    if start > end {
                        return Err(Failure::new(
                            ErrorCode::InvalidRequest,
                            "Reversed signature label span",
                        )
                        .into());
                    }
                    signature.label[start..end].to_string()
                }
            };
            Ok(direct::Parameter {
                label,
                documentation: parameter.documentation.map(plain_documentation),
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(direct::Signature {
        label: signature.label,
        documentation: signature.documentation.map(plain_documentation),
        active_parameter: parameter_index(signature.active_parameter, parameters.len()),
        parameters,
    })
}

/// An empty parameter list has no active parameter; an invalid supplied index defaults to zero.
fn parameter_index(index: Option<u32>, count: usize) -> Option<u32> {
    if count == 0 {
        None
    } else {
        index.map(|index| if (index as usize) < count { index } else { 0 })
    }
}

/// Markup documentation stays inert; requesting plaintext does not authorize rendering server HTML.
fn plain_documentation(documentation: lsp_types::Documentation) -> String {
    match documentation {
        lsp_types::Documentation::String(text) => text,
        lsp_types::Documentation::MarkupContent(content) => content.value,
    }
}

/// Reject a label endpoint inside a surrogate pair, including for multiline signature labels.
fn label_offset(label: &str, offset: u32) -> anyhow::Result<usize> {
    let mut units = 0;
    for (byte, character) in label.char_indices() {
        if units == offset {
            return Ok(byte);
        }
        units += character.len_utf16() as u32;
    }
    if units == offset {
        return Ok(label.len());
    }
    Err(Failure::new(
        ErrorCode::InvalidRequest,
        "Signature offset is outside label or splits a surrogate pair",
    )
    .into())
}
