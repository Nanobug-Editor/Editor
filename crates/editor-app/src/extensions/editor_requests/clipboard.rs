//! Multi-format clipboard IO runs off the UI thread and retains the original runtime request gate.
use super::*;
use plugin_runtime::plugin_protocol::transfer::{
    self, ClipboardOperation as Operation, ClipboardValue, Format, Item,
};

impl EditorApp {
    /// Execute a separately negotiated clipboard call; a delayed OS provider never stalls native input.
    pub(super) fn perform_formatted_clipboard(
        &mut self,
        request: EditorRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        #[cfg(windows)]
        let owner = {
            use raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
            window
                .window_handle()
                .ok()
                .and_then(|handle| match handle.as_raw() {
                    RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
                    _ => None,
                })
                .unwrap_or(0)
        };
        #[cfg(not(windows))]
        let owner = 0;
        let work = request.clone();
        cx.spawn(async move |_, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { perform(&work, owner) })
                .await;
            request.finish(result);
        })
        .detach();
    }
}

/// All payload selection, decoding and side effects still belong to this original admitted request.
#[cfg(windows)]
fn perform(request: &EditorRequest, owner: isize) -> Result<Value, Failure> {
    use platform_windows::clipboard;
    let Op::Clipboard { operation } = request.operation() else {
        unreachable!()
    };
    let result = match operation {
        Operation::Enumerate => {
            let offer = request.clipboard_offer().ok_or_else(invalid_offer)?;
            let snapshot = clipboard::enumerate(owner).map_err(native)?;
            let formats = snapshot
                .formats
                .into_iter()
                .map(|mime| Format {
                    mime,
                    byte_len: None,
                })
                .collect();
            ClipboardValue::Offer(offer.publish(snapshot.sequence, formats)?)
        }
        Operation::Read { preferred, .. } => {
            let offer = request.clipboard_offer().ok_or_else(invalid_offer)?;
            let (sequence, mime) = offer.select(preferred)?;
            let item = Item {
                bytes: clipboard::read(owner, sequence, &mime, transfer::MAX_BYTES)
                    .map_err(native)?,
                mime,
            };
            validate_item(&item)?;
            if !offer.is_live() || !request.authority().is_live() {
                return Err(invalid_offer());
            }
            ClipboardValue::Data(item)
        }
        Operation::Write { items } => {
            for item in items {
                validate_item(item)?;
            }
            let native_items = items
                .iter()
                .map(|item| (item.mime.clone(), item.bytes.clone()))
                .collect::<Vec<_>>();
            // Retired, cancelled or expired writes cannot replace the user's current clipboard.
            if !request.enter_side_effect() {
                return Err(Failure::new(
                    ErrorCode::Cancelled,
                    "Clipboard write cancelled",
                ));
            }
            clipboard::write(owner, &native_items, transfer::MAX_BYTES).map_err(native)?;
            ClipboardValue::Written
        }
    };
    Ok(Value::ClipboardFormats(result))
}

/// Use the same binary image checks and pixel limits as existing native document image input.
pub(super) fn validate_item(item: &Item) -> Result<(), Failure> {
    item.validate()?;
    if item.mime == "image/png"
        && crate::ui::plugin::bitmap::input_format(&item.bytes)? != api::ImageFormat::Png
    {
        return Err(Failure::new(
            ErrorCode::InvalidRequest,
            "PNG clipboard format requires PNG bytes",
        ));
    }
    Ok(())
}

fn invalid_offer() -> Failure {
    Failure::new(
        ErrorCode::InvalidHandle,
        "Clipboard offer is not live in this instance",
    )
}

#[cfg(windows)]
fn native(error: platform_windows::clipboard::Error) -> Failure {
    use platform_windows::clipboard::Error;
    Failure::new(
        match error {
            Error::Changed => ErrorCode::StaleRevision,
            Error::Limit => ErrorCode::LimitExceeded,
            Error::Invalid => ErrorCode::InvalidRequest,
            Error::Unavailable(_) => ErrorCode::OperationFailed,
        },
        error.to_string(),
    )
}

/// Other OS adapters must report their actual availability before the host advertises this interface.
#[cfg(not(windows))]
fn perform(_: &EditorRequest, _: isize) -> Result<Value, Failure> {
    Err(Failure::new(
        ErrorCode::UnsupportedOperation,
        "Formatted clipboard is unavailable on this platform",
    ))
}
