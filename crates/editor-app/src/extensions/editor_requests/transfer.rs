//! Native drop reads materialize only one chosen inert representation on the existing worker pool.
use super::*;

impl EditorApp {
    /// Preserve the original owner/deadline while an OS data provider prepares its selected bytes.
    pub(super) fn perform_transfer_read(&mut self, request: EditorRequest, cx: &mut Context<Self>) {
        let read = request.clone();
        cx.spawn(async move |_, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let Op::Transfer { operation } = read.operation() else {
                        unreachable!()
                    };
                    let offer = read.transfer_offer().ok_or_else(|| {
                        Failure::new(ErrorCode::InvalidHandle, "No admitted transfer offer")
                    })?;
                    use plugin_runtime::plugin_protocol::transfer::{
                        Operation, Value as TransferValue,
                    };
                    let value = match operation {
                        Operation::Read { preferred, .. } => offer.read_selected(preferred)?,
                        Operation::ReadFile {
                            file,
                            offset,
                            max_bytes,
                        } => offer.read_file(file, *offset, *max_bytes)?,
                    };
                    if let TransferValue::Data(item) = &value {
                        super::clipboard::validate_item(item)?;
                    }
                    Ok(Value::Transfer(value))
                })
                .await;
            request.finish(result);
        })
        .detach();
    }
}
