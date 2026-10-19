//! Runtime-sealed language requests reuse one context transport and never expose native module paths.
use super::*;
use futures::FutureExt as _;
use plugin_runtime::plugin_protocol::language_control::{
    MAX_CONTROL_BYTES, NativeControlFailureData,
};

/// Immutable host capture from the original native source; no extra mutable text model is created.
/// This travels only over LSP's source channel and never through the bounded guest control payload.
struct ControlSource {
    document: api::DocumentVersion,
    path: PathBuf,
    language: String,
    text: String,
}

impl EditorApp {
    /// Adopt the current host generation atomically, prepare that original provider/context, then
    /// send one bounded declared control operation. Scope changes, cancellation and owner retirement
    /// stop waits and reject delivery; arguments and metadata are never sampled from later focus.
    pub(super) fn perform_language_control(
        &mut self,
        request: EditorRequest,
        cx: &mut Context<Self>,
    ) {
        let Op::LanguageRequest { request: control } = request.operation() else {
            unreachable!()
        };
        let mut control = control.clone();
        let Some(metadata) = request.language_control_metadata().cloned() else {
            request.finish(Err(Failure::new(
                ErrorCode::PermissionDenied,
                "Language control has no runtime authority",
            )));
            return;
        };
        let plugin = metadata.provider.split('/').next().unwrap();
        let Some(availability) = self.extensions.read(cx).availability(plugin) else {
            request.finish(Err(Failure::new(
                ErrorCode::CapabilityUnavailable,
                "Language control provider is unavailable",
            )));
            return;
        };
        if control.document.generation != 0
            && control.document.generation != availability.configuration_generation()
        {
            request.finish(Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Language control configuration was superseded",
            )));
            return;
        }
        control.document.generation = availability.configuration_generation();
        let source = self.tabs.iter().enumerate().find_map(|(index, tab)| {
            let relative = tab
                .path()
                .strip_prefix(self.workspace.root())
                .ok()?
                .to_string_lossy()
                .replace('\\', "/");
            if relative != control.document.path {
                return None;
            }
            let document = self.plugin_document_version(index).ok()?;
            let state = tab.text.as_ref()?.editor.read(cx);
            if state.text().len() > 32 * 1024 * 1024 {
                return Some(Err(Failure::new(
                    ErrorCode::LimitExceeded,
                    "Native language source exceeds 32 MiB",
                )));
            }
            Some(Ok(ControlSource {
                document,
                path: tab.path().to_owned(),
                language: self.language_for_path(tab.path()),
                text: state.text().to_string(),
            }))
        });
        let source = match source.transpose() {
            Ok(source) => source,
            Err(error) => {
                request.finish(Err(error));
                return;
            }
        };
        let lease = self.acquire_language_document_lease(&metadata.provider, &control.document, cx);
        let mut wait = match self.extensions.read(cx).demand_language_document(
            &metadata.provider,
            control.document.clone(),
            plugin_runtime::LanguageRole::Lsp,
            None,
        ) {
            Ok(wait) => wait,
            Err(error) => {
                request.finish(Err(error));
                return;
            }
        };
        cx.spawn(async move |this,cx| {
            let _lease = lease;
            loop {
                if request.status().is_terminal() {wait.cancellation().cancel();return;}
                if !this.update(cx,|app,_|source.as_ref().is_none_or(|source|app.control_source_is_current(source))).unwrap_or(false) {
                    wait.cancellation().cancel();
                    request.finish(Err(Failure::new(ErrorCode::StaleRevision,"Language control source changed during preparation")));
                    return;
                }
                if let Some(result) = wait.try_take() {
                    if let Err(error) = result {request.finish(Err(error));return;}
                    break;
                }
                cx.background_executor().timer(Duration::from_millis(10)).await;
            }
            // An actor receipt may precede its immutable publication by one tick. Wait for only
            // this exact binding; a same-language/default service is never a valid substitute.
            let (server,plan) = loop {
                if request.status().is_terminal() {return;}
                let resolved = this.update(cx,|app,cx| -> Result<_,Failure> {
                    if !app.session_state.workspace_trusted || app.workspace.root() != Path::new(request.workspace()).canonicalize().unwrap_or_default()
                        || source.as_ref().is_some_and(|source|!app.control_source_is_current(source))
                        || app.extensions.read(cx).availability(availability.plugin()).as_ref() != Some(&availability) {
                        return Err(Failure::new(ErrorCode::InvalidHandle,"Language control authority changed"));
                    }
                    let Some(plan) = app.extensions.read(cx).language_document_service(&metadata.provider,&control.document) else {return Ok(None);};
                    let plan = plan.map_err(|error|Failure::new(ErrorCode::OperationFailed,error))?;
                    request.register_language_consumer(&plan)?;
                    Ok(app.acquire_language_context_server(plan.clone()).map(|server|(server,plan)))
                });
                match resolved {
                    Ok(Ok(Some(value)))=>break value,
                    Ok(Ok(None))=>cx.background_executor().timer(Duration::from_millis(10)).await,
                    Ok(Err(error))=>{request.finish(Err(error));return;},
                    Err(_)=>{request.finish(Err(Failure::new(ErrorCode::InvalidHandle,"Language control window closed")));return;}
                }
            };
            let imports = metadata.imports.iter().map(|lease| {
                let descriptor = lease.descriptor();
                serde_json::json!({"export_id":descriptor.export_id,"entry":descriptor.entry,
                    "package_root":descriptor.package_root,"package_digest":descriptor.package_digest,
                    "sha256":descriptor.sha256,"abi":descriptor.abi,"engine":descriptor.engine,
                    "files":descriptor.files,"module_root":descriptor.module_root})
            }).collect::<Vec<_>>();
            let parameters = serde_json::json!({"arguments":control.arguments,
                "_nanobug":{"owner":metadata.owner,"consumer":metadata.consumer,"imports":imports}});
            let cancellation = plugin_runtime::RequestCancellation::default();
            let context = if control.context_snapshot {
                let captured = this.update(cx, |app,cx| {
                    let source = source.as_ref().ok_or_else(||Failure::new(ErrorCode::InvalidHandle,
                        "Context analysis requires an open native source"))?;
                    app.capture_language_context(&server,&source.document,&source.text,cx)?
                        .ok_or_else(||Failure::new(ErrorCode::UnsupportedOperation,
                            "Provider has no related source declaration"))
                });
                match captured {
                    Ok(Ok(context)) => Some(context.with_cancellation(cancellation.clone())),
                    Ok(Err(error))=>{request.finish(Err(error));return;},
                    Err(_)=>{request.finish(Err(Failure::new(ErrorCode::InvalidHandle,"Language window closed")));return;}
                }
            } else { None };
            let response_context = context.clone();
            let worker_cancel = cancellation.clone();
            let worker_request = request.clone();
            let publish_binding = control.publish_binding.is_some();
            let native_source = source.as_ref().and_then(|source|crate::language::navigation::file_uri(&source.path)
                .map(|uri|(server.open_document_for_language(uri,&source.language),source.text.clone(),source.document.clone())));
            let worker_imports = metadata.imports.clone();
            let worker_plan = plan.clone();
            let mut job = cx.background_executor().scheduler_executor().spawn_dedicated(move |_| async move {
                if !worker_request.enter_side_effect() {return Err(anyhow::anyhow!("Language control was cancelled before dispatch"));}
                // A failed native capability ACK cannot leave speculative executable dependencies
                // on a healthy plan. Reservations remain inert until this operation can be sent.
                server.prepare_until_ready()?;
                let mut attachments = worker_imports.iter().map(|module|module.reserve(&worker_plan))
                    .collect::<Result<Vec<_>,_>>()?;
                let operation = || {
                    let mut parameters = parameters;
                    if let Some(context) = &context {
                        let sources = context.wire_sources().ok_or_else(||anyhow::anyhow!("Language sources were not synchronized"))?;
                        if sources.len() > plugin_runtime::plugin_protocol::language_context::MAX_CANDIDATES {
                            return Err(Failure::new(ErrorCode::LimitExceeded,"Too many control source snapshots").into());
                        }
                        parameters["_nanobug"]["sources"] = serde_json::json!(sources);
                    }
                    for index in 0..attachments.len() {
                        if let Err(error) = attachments[index].dispatch() {
                            // Nothing from this operation has entered the native pipe yet. This
                            // known admission failure releases only these provisional reservations.
                            for attachment in attachments.drain(..) {attachment.release();}
                            return Err(error.into());
                        }
                    }
                    match native_source {
                        Some((document,text,version))=>server.control_request_at_version(&control.method,parameters,document,&text,&version,&worker_cancel),
                        None=>server.control_request(&control.method,parameters,&worker_cancel),
                    }
                };
                let result = match &context {
                    Some(context)=>server.with_context_snapshot(context,operation),
                    None=>operation(),
                };
                let released = result.as_ref().err().and_then(|error|error.downcast_ref::<NativeControlFailureData>())
                    .is_some_and(NativeControlFailureData::valid);
                // A matching receipt concerns this request's new uses, never another accepted or
                // parallel use. Timeout/cancellation/ordinary errors leave possible loads pinned.
                for attachment in attachments {
                    if released {attachment.release();} else {attachment.commit();}
                }
                result
            });
            let result = loop {
                if request.status().is_terminal() {cancellation.cancel();return;}
                if !this.update(cx,|app,_|source.as_ref().is_none_or(|source|app.control_source_is_current(source))).unwrap_or(false) {
                    cancellation.cancel();
                    request.finish(Err(Failure::new(ErrorCode::StaleRevision,"Language control source changed during analysis")));
                    return;
                }
                if let Some(result) = (&mut job).now_or_never() {break result;}
                cx.background_executor().timer(Duration::from_millis(10)).await;
            };
            let result = result.map_err(|error| error.downcast_ref::<Failure>().cloned()
                .unwrap_or_else(||Failure::new(ErrorCode::OperationFailed,format!("{error:#}"))));
            let admitted = this.update(cx,|app,cx| {
                if !app.session_state.workspace_trusted || !plan.is_active()
                    || source.as_ref().is_some_and(|source|!app.control_source_is_current(source))
                    || app.extensions.read(cx).availability(availability.plugin()).as_ref() != Some(&availability)
                    || !metadata.imports.iter().all(|lease|lease.is_live()) {
                    return Err(Failure::new(ErrorCode::InvalidHandle,"Language control context was retired"));
                }
                let value = result?;
                if response_context.as_ref().is_some_and(|context|!app.language_context_is_current(context,cx)) {
                    return Err(Failure::new(ErrorCode::StaleRevision,"Language control project snapshot changed"));
                }
                if serde_json::to_vec(&value).map_or(true,|bytes|bytes.len() > MAX_CONTROL_BYTES) {
                    return Err(Failure::new(ErrorCode::LimitExceeded,"Language control response exceeds 64 KiB"));
                }
                metadata.response.accepts(&value)?;
                if let Some(source) = &source {
                    app.retain_control_source_context(&metadata.provider, &control.document,
                        &source.document, request.authority(), cx)?;
                }
                Ok(value)
            });
            match admitted.unwrap_or_else(|_|Err(Failure::new(ErrorCode::InvalidHandle,"Language control window closed"))) {
                Ok(value) if publish_binding => request.finish_language_binding(value,plan),
                result => request.finish(result.map(Value::Language)),
            }
        }).detach();
    }

    /// Verify the original source incarnation and revision without reading replacement text or focus.
    fn control_source_is_current(&self, source: &ControlSource) -> bool {
        self.tabs.iter().enumerate().any(|(index, tab)| {
            tab.path() == source.path
                && self.plugin_document_version(index).ok().as_ref() == Some(&source.document)
        })
    }
}
