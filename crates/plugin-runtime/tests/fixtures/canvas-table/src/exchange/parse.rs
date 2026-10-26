//! Bounded table-only data parsing: no document renderer, URL loading or script execution exists here.

/// Parse a finite TSV matrix, including escaped double quotes and embedded quoted tabs/newlines.
pub(super) fn tsv(text: &str) -> Result<Vec<Vec<String>>, &'static str> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut input = text.chars().peekable();
    while let Some(ch) = input.next() {
        match ch {
            '"' if quoted && input.peek() == Some(&'"') => {
                input.next();
                field.push('"');
            }
            '"' if quoted => quoted = false,
            '"' if field.is_empty() => quoted = true,
            '\t' if !quoted => {
                row.push(std::mem::take(&mut field));
                if row.len() > 2 {
                    return Err("too many columns");
                }
            }
            '\n' | '\r' if !quoted => {
                if ch == '\r' && input.peek() == Some(&'\n') {
                    input.next();
                }
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
                if rows.len() > 3 {
                    return Err("too many rows");
                }
            }
            _ => field.push(ch),
        }
    }
    if quoted {
        return Err("unterminated TSV quote");
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    Ok(rows)
}
/// Accept table row/cell markup only. Tags inside a cell contribute plain text, never executable nodes.
pub(super) fn html(text: &str) -> Result<Vec<Vec<String>>, &'static str> {
    let lower = text.to_ascii_lowercase();
    let mut cursor = 0;
    let mut rows = Vec::new();
    while let Some(start) = tag(&lower[cursor..], "tr") {
        let start = cursor + start;
        let content = lower[start..].find('>').ok_or("invalid HTML row")? + start + 1;
        let end = lower[content..].find("</tr>").ok_or("unclosed HTML row")? + content;
        let mut at = content;
        let mut row = Vec::new();
        while at < end {
            let cell = tag(&lower[at..end], "td")
                .map(|offset| (at + offset, "</td>"))
                .into_iter()
                .chain(tag(&lower[at..end], "th").map(|offset| (at + offset, "</th>")))
                .min_by_key(|(offset, _)| *offset);
            let Some((start, closing)) = cell else {
                break;
            };
            let begin = lower[start..end].find('>').ok_or("invalid HTML cell")? + start + 1;
            let finish = lower[begin..end]
                .find(closing)
                .ok_or("unclosed HTML cell")?
                + begin;
            row.push(plain(&text[begin..finish]));
            if row.len() > 2 {
                return Err("too many columns");
            }
            at = finish + closing.len();
        }
        rows.push(row);
        if rows.len() > 3 {
            return Err("too many rows");
        }
        cursor = end + 5;
    }
    Ok(rows)
}
/// Match a complete start-tag name, so containers such as thead are never mistaken for cells.
fn tag(text: &str, name: &str) -> Option<usize> {
    let needle = format!("<{name}");
    text.match_indices(&needle).find_map(|(at, _)| {
        text.as_bytes()
            .get(at + needle.len())
            .is_some_and(|next| *next == b'>' || next.is_ascii_whitespace())
            .then_some(at)
    })
}
/// Strip formatting and decode the finite entities emitted by the table serializer.
fn plain(text: &str) -> String {
    let mut output = String::new();
    let mut tag = false;
    for ch in text.chars() {
        match ch {
            '<' => tag = true,
            '>' => tag = false,
            _ if !tag => output.push(ch),
            _ => {}
        }
    }
    output
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
}
