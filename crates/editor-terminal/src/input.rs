//! Xterm keyboard encoding; printable text is delivered separately by the platform IME.

/// Encode navigation/control keys, preserving application cursor mode and modifiers.
pub fn key(key: &str, ctrl: bool, alt: bool, shift: bool, app_cursor: bool) -> Option<Vec<u8>> {
    let modifier = 1 + u8::from(shift) + 2 * u8::from(alt) + 4 * u8::from(ctrl);
    let cursor = match key {
        "up" => Some("A"),
        "down" => Some("B"),
        "right" => Some("C"),
        "left" => Some("D"),
        "home" => Some("H"),
        "end" => Some("F"),
        _ => None,
    };
    if let Some(final_byte) = cursor {
        return Some(
            if modifier != 1 {
                format!("\x1b[1;{modifier}{final_byte}")
            } else if app_cursor {
                format!("\x1bO{final_byte}")
            } else {
                format!("\x1b[{final_byte}")
            }
            .into_bytes(),
        );
    }
    let tilde = match key {
        "insert" => Some(2),
        "delete" => Some(3),
        "pageup" => Some(5),
        "pagedown" => Some(6),
        "f5" => Some(15),
        "f6" => Some(17),
        "f7" => Some(18),
        "f8" => Some(19),
        "f9" => Some(20),
        "f10" => Some(21),
        "f11" => Some(23),
        "f12" => Some(24),
        "f13" => Some(25),
        "f14" => Some(26),
        "f15" => Some(28),
        "f16" => Some(29),
        "f17" => Some(31),
        "f18" => Some(32),
        "f19" => Some(33),
        "f20" => Some(34),
        _ => None,
    };
    if let Some(code) = tilde {
        return Some(
            if modifier == 1 {
                format!("\x1b[{code}~")
            } else {
                format!("\x1b[{code};{modifier}~")
            }
            .into_bytes(),
        );
    }
    if let Some(code) = match key {
        "f1" => Some('P'),
        "f2" => Some('Q'),
        "f3" => Some('R'),
        "f4" => Some('S'),
        _ => None,
    } {
        return Some(
            if modifier == 1 {
                format!("\x1bO{code}")
            } else {
                format!("\x1b[1;{modifier}{code}")
            }
            .into_bytes(),
        );
    }
    let mut bytes = match key {
        "enter" => vec![b'\r'],
        "backspace" => vec![if ctrl { 8 } else { 127 }],
        "escape" => vec![27],
        "tab" if shift => b"\x1b[Z".to_vec(),
        "tab" => vec![9],
        "space" | "2" if ctrl => vec![0],
        "3" if ctrl => vec![27],
        "4" if ctrl => vec![28],
        "5" if ctrl => vec![29],
        "6" if ctrl => vec![30],
        "7" | "/" if ctrl => vec![31],
        "8" | "?" if ctrl => vec![127],
        _ if ctrl && key.len() == 1 => {
            let byte = key.as_bytes()[0].to_ascii_uppercase();
            if (b'@'..=b'_').contains(&byte) {
                vec![byte & 0x1f]
            } else {
                return None;
            }
        }
        _ if alt && key.chars().count() == 1 => key.as_bytes().to_vec(),
        _ => return None,
    };
    if alt {
        bytes.insert(0, 27);
    }
    Some(bytes)
}

/// Encode one already-authorized Xterm mouse report. Coordinates are one-based native screen cells.
/// `modifiers` uses Xterm's shift/alt/control bits (4/8/16); the view retains Shift selection override.
pub fn mouse(
    button: u8,
    column: usize,
    row: usize,
    release: bool,
    sgr: bool,
    utf8: bool,
    modifiers: u8,
) -> Option<Vec<u8>> {
    let button = button | modifiers;
    if sgr {
        return Some(
            format!(
                "\x1b[<{button};{column};{row}{}",
                if release { 'm' } else { 'M' }
            )
            .into_bytes(),
        );
    }
    let button = 32 + if release { 3 | modifiers } else { button };
    if utf8 {
        if column > 2015 || row > 2015 {
            return None;
        }
        let mut value = String::from("\x1b[M");
        value.push(char::from(button));
        value.push(char::from_u32(32 + column as u32)?);
        value.push(char::from_u32(32 + row as u32)?);
        Some(value.into_bytes())
    } else if column <= 223 && row <= 223 {
        Some(vec![
            27,
            b'[',
            b'M',
            button,
            32 + column as u8,
            32 + row as u8,
        ])
    } else {
        None
    }
}

/// Bracketed paste prevents multiline pastes from being interpreted as individual key presses.
pub fn paste(text: &str, bracketed: bool) -> Vec<u8> {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    if bracketed {
        format!("\x1b[200~{}\x1b[201~", text.replace('\x1b', "")).into_bytes()
    } else {
        text.replace('\n', "\r").into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Encode raw control bytes and application arrows; the UI intercepts clipboard keys first.
    #[test]
    fn control_and_navigation() {
        assert_eq!(key("c", true, false, false, false), Some(vec![3]));
        assert_eq!(
            key("up", false, false, false, true),
            Some(b"\x1bOA".to_vec())
        );
        assert_eq!(
            key("left", true, true, false, false),
            Some(b"\x1b[1;7D".to_vec())
        );
        assert_eq!(key("a", false, false, false, false), None);
    }
    /// Embedded escape sequences cannot terminate bracketed paste prematurely.
    #[test]
    fn paste_is_delimited() {
        assert_eq!(
            paste("a\r\nb\x1b[201~", true),
            b"\x1b[200~a\nb[201~\x1b[201~"
        );
    }

    /// Extended function/control keys and UTF-8 mouse coordinates reach the child with Xterm semantics.
    #[test]
    fn extended_keys_and_mouse_protocols() {
        assert_eq!(
            key("f20", false, false, false, false),
            Some(b"\x1b[34~".to_vec())
        );
        assert_eq!(key("2", true, false, false, false), Some(vec![0]));
        assert_eq!(key("?", true, false, false, false), Some(vec![127]));
        assert_eq!(key("enter", false, false, true, false), Some(vec![13]));
        assert_eq!(
            mouse(0, 300, 12, false, true, false, 16),
            Some(b"\x1b[<16;300;12M".to_vec())
        );
        assert_eq!(mouse(0, 300, 12, false, false, false, 0), None);
        assert_eq!(
            String::from_utf8(mouse(0, 300, 12, false, false, true, 0).unwrap()).unwrap(),
            "\x1b[M Ō,"
        );
        assert_eq!(
            mouse(0, 1, 1, true, true, false, 8),
            Some(b"\x1b[<8;1;1m".to_vec())
        );
    }
}
