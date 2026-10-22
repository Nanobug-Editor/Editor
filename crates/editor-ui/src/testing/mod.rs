//! Native input helpers shared by headless regressions and bounded Windows acceptance.
//! Every event uses the original window handler; no text buffer or focus state is synthesized.
use gpui_kit::{App, KeyDownEvent, KeyUpEvent, Keystroke, PlatformInput, Window};

/// Deliver Unicode `text` to `window` while `cx` owns its update, without advancing the executor.
/// Refresh each character's frame so input handlers track any focus change during dispatch.
pub fn input_text(window: &mut Window, text: &str, cx: &mut App) {
    window.refresh();
    window.draw(cx).clear(cx);
    for character in text.chars() {
        let text = character.to_string();
        let mut key = Keystroke::parse(&text).expect("Unicode text is a valid GPUI keystroke");
        key.key_char = Some(text);
        window.dispatch_keystroke(key, cx);
        window.refresh();
        window.draw(cx).clear(cx);
    }
}

/// Send the parsed native `key` pair to `window` in `cx`; invalid test notation panics.
/// Enter and Tab dispatch bindings without adding simulated IME text afterward.
pub fn press_key(window: &mut Window, key: &str, cx: &mut App) {
    let key = Keystroke::parse(key).expect("the acceptance keystroke is valid");
    window.refresh();
    window.draw(cx).clear(cx);
    let key = if matches!(key.key.as_str(), "enter" | "tab") {
        window.dispatch_event(
            PlatformInput::KeyDown(KeyDownEvent {
                keystroke: key.clone(),
                is_held: false,
                prefer_character_input: false,
            }),
            cx,
        );
        key
    } else {
        let key = key.with_simulated_ime();
        window.dispatch_keystroke(key.clone(), cx);
        key
    };
    window.dispatch_event(PlatformInput::KeyUp(KeyUpEvent { keystroke: key }), cx);
    window.refresh();
    window.draw(cx).clear(cx);
}
