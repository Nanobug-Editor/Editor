//! Bounded one-shot frame requests share atomic publication and the original native owner lifetime.
use super::*;
use plugin_protocol::{api, ui};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, atomic::AtomicU64};

const MAX_PENDING: usize = 128;
type Key = (String, String);

/// Source and overlay ancestry remain independent of drawing-only scene revisions.
#[derive(Clone, PartialEq)]
struct Owner {
    identity: u64,
    source: Option<api::DocumentVersion>,
    file: Option<api::FileVersion>,
    lineage: crate::ui_owners::Lineage,
}
impl Owner {
    fn current(document: &ui::Document, canvas: &str) -> Option<Self> {
        let node = crate::ui_owners::find(document, canvas)?;
        let ui::Kind::Canvas(model) = &node.kind else {
            return None;
        };
        (!node.disabled).then_some(Self {
            identity: model.scene.as_ref()?.identity,
            source: document.source.clone(),
            file: document.file.clone(),
            lineage: crate::ui_owners::lineage(document, canvas)?,
        })
    }
}
struct Pending {
    owner: Owner,
    resource: Arc<crate::CanvasFrameResource>,
}
/// Retained state is bounded by current declared canvases and 128 outstanding requests per instance.
#[derive(Default)]
pub(super) struct FrameRequests {
    pending: BTreeMap<Key, Pending>,
    last: BTreeMap<Key, (Owner, u64)>,
    sequence: Arc<AtomicU64>,
}
impl FrameRequests {
    fn reconcile(&mut self, views: &BTreeMap<String, Arc<ui::Document>>) {
        self.pending.retain(|(panel, canvas), pending| {
            let live = views
                .get(panel)
                .and_then(|document| Owner::current(document, canvas))
                .as_ref()
                == Some(&pending.owner);
            if !live {
                pending.resource.revoke();
            }
            live
        });
        self.last.retain(|(panel, canvas), (owner, _)| {
            views
                .get(panel)
                .and_then(|document| Owner::current(document, canvas))
                .as_ref()
                == Some(owner)
        });
    }
}
impl Drop for FrameRequests {
    fn drop(&mut self) {
        for pending in self.pending.values() {
            pending.resource.revoke();
        }
    }
}
impl Instance {
    /// Read-only admission participates in the same all-or-nothing Output validation as its views.
    pub(super) fn validate_frame_requests(
        &self,
        requests: &[ui::CanvasFrameRequest],
        views: &[api::View],
    ) -> Result<(), api::Failure> {
        if requests.is_empty() {
            return Ok(());
        }
        if !self
            .negotiated()
            .capabilities
            .get("ui.canvas")
            .is_some_and(|v| *v >= semver::Version::new(1, 10, 0))
        {
            return Err(api::Failure::new(
                api::ErrorCode::CapabilityUnavailable,
                "Canvas frame requests require ui.canvas 1.10",
            ));
        }
        if requests.len() > MAX_PENDING {
            return Err(api::Failure::new(
                api::ErrorCode::LimitExceeded,
                "Too many Canvas frame requests",
            ));
        }
        let mut keys = BTreeSet::new();
        let future = |panel: &str| {
            views
                .iter()
                .find(|view| view.panel == panel)
                .map(|view| &view.document)
                .or_else(|| self.views.get(panel).map(Arc::as_ref))
        };
        let mut count = self
            .frame_requests
            .pending
            .iter()
            .filter(|((panel, canvas), pending)| {
                future(panel)
                    .and_then(|document| Owner::current(document, canvas))
                    .as_ref()
                    == Some(&pending.owner)
            })
            .count();
        for request in requests {
            request.validate()?;
            let key = (request.panel.clone(), request.canvas.clone());
            if !self.panels.contains(&request.panel) || !keys.insert(key.clone()) {
                return Err(api::Failure::new(
                    api::ErrorCode::InvalidRequest,
                    "Undeclared or duplicate Canvas frame target",
                ));
            }
            let owner = future(&request.panel)
                .and_then(|document| Owner::current(document, &request.canvas))
                .filter(|owner| owner.identity == request.identity)
                .ok_or_else(|| {
                    api::Failure::new(
                        api::ErrorCode::StaleRevision,
                        "Canvas frame owner is unavailable",
                    )
                })?;
            if let Some(pending) = self
                .frame_requests
                .pending
                .get(&key)
                .filter(|pending| pending.owner == owner)
            {
                if pending.resource.request() == request {
                    continue;
                }
                return Err(api::Failure::new(
                    api::ErrorCode::Conflict,
                    "A Canvas frame request is already pending",
                ));
            }
            if self
                .frame_requests
                .last
                .get(&key)
                .is_some_and(|(old, token)| *old == owner && request.request <= *token)
            {
                return Err(api::Failure::new(
                    api::ErrorCode::StaleRevision,
                    "Canvas frame request token did not advance",
                ));
            }
            count += 1;
            if count > MAX_PENDING {
                return Err(api::Failure::new(
                    api::ErrorCode::LimitExceeded,
                    "Canvas frame request quota exceeded",
                ));
            }
        }
        Ok(())
    }
    /// Commit cannot fail after readonly validation and atomic view publication.
    pub(super) fn accept_frame_requests(&mut self, requests: &[ui::CanvasFrameRequest]) {
        self.frame_requests.reconcile(&self.views);
        for request in requests {
            let key = (request.panel.clone(), request.canvas.clone());
            if self.frame_requests.pending.contains_key(&key) {
                continue;
            }
            let owner = Owner::current(&self.views[&request.panel], &request.canvas)
                .expect("validated Canvas frame owner");
            let resource = Arc::new(crate::CanvasFrameResource::new(
                request.clone(),
                self.store.data().plugin_services.alive.clone(),
                self.frame_requests.sequence.clone(),
                &self.views[&request.panel],
            ));
            self.frame_requests
                .last
                .insert(key.clone(), (owner.clone(), request.request));
            self.frame_requests
                .pending
                .insert(key, Pending { owner, resource });
        }
    }
    /// Native adapters borrow pending leases; no dequeue can lose a hidden request.
    pub(crate) fn canvas_frame_requests(&self) -> Vec<Arc<crate::CanvasFrameResource>> {
        self.frame_requests
            .pending
            .values()
            .filter(|pending| pending.resource.is_pending())
            .map(|pending| pending.resource.clone())
            .collect()
    }
    /// An actual native completion is consumed before invoking guest code; timers remain independent.
    pub(super) fn poll_canvas_frames(&mut self) -> anyhow::Result<bool> {
        self.frame_requests.reconcile(&self.views);
        let completed = self
            .frame_requests
            .pending
            .iter()
            .filter_map(|(key, pending)| {
                pending
                    .resource
                    .take()
                    .map(|opportunity| (key.clone(), opportunity))
            })
            .collect::<Vec<_>>();
        let changed = !completed.is_empty();
        for ((panel, canvas), opportunity) in completed {
            if let Some(pending) = self
                .frame_requests
                .pending
                .remove(&(panel.clone(), canvas.clone()))
            {
                pending.resource.revoke();
            }
            self.notify(
                Some(panel.clone()),
                api::Notification::CanvasFrame {
                    panel,
                    canvas,
                    opportunity,
                },
            )?;
        }
        Ok(changed)
    }
}
