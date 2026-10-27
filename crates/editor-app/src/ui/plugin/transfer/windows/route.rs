//! Metadata-only OS hit routing retains the original actor, semantic lifetime and local coordinates.
use super::*;
use plugin_runtime::{
    HostTransferData, TransferReader,
    plugin_protocol::transfer::{self, Format, Item, Origin},
};

/// Native geometry belongs to one successful paint; target authority is an independently revocable Arc.
#[derive(Clone)]
pub(super) struct Entry {
    pub address: Address,
    pub bounds: [f32; 4],
    pub origin: [f32; 2],
    pub scale: f32,
    pub dpi: f32,
    pub preferred: Vec<String>,
    pub alive: Arc<AtomicBool>,
    pub sink: Sink,
    pub order: u64,
    pub path: Vec<usize>,
    pub frame: Option<crate::ui::plugin::canvas::TransferFrame>,
    pub native_drop: bool,
}
impl Entry {
    fn contains(&self, point: drag::Point) -> bool {
        let [x, y, w, h] = self.bounds;
        let (px, py) = (point.x as f32 / self.dpi, point.y as f32 / self.dpi);
        self.alive.load(Ordering::Acquire) && px >= x && py >= y && px < x + w && py < y + h
    }
    fn position(&self, point: drag::Point) -> Position {
        if self.address.canvas.is_some()
            && let Some(frame) = &self.frame
        {
            return frame.position(point.x as f32 / self.dpi, point.y as f32 / self.dpi);
        }
        Position {
            x: (point.x as f32 / self.dpi - self.origin[0]) / self.scale,
            y: (point.y as f32 / self.dpi - self.origin[1]) / self.scale,
        }
    }
    fn resolve(&self, point: drag::Point) -> Option<Self> {
        let mut entry = self.clone();
        if let Some(frame) = &self.frame {
            if let Some((anchor, target)) =
                frame.drop_at(point.x as f32 / self.dpi, point.y as f32 / self.dpi)
            {
                entry.address.canvas = Some(anchor);
                entry.preferred = target.preferred;
            } else if !self.native_drop {
                return None;
            }
        } else if !self.native_drop {
            return None;
        }
        Some(entry)
    }
}
struct Active {
    serial: u64,
    entry: Entry,
    last: Position,
}

/// One window has one OS drag; post-drop resources remain exclusively in the runtime's finite registry.
pub(super) struct State {
    pub owners: BTreeMap<u64, Owner>,
    pub entries: BTreeMap<(u64, String), Entry>,
    pub sequence: u64,
    active: Option<Active>,
    fallback: futures::channel::mpsc::Sender<fallback::Legacy>,
}
impl State {
    pub(super) fn new(fallback: futures::channel::mpsc::Sender<fallback::Legacy>) -> Self {
        Self {
            owners: BTreeMap::new(),
            entries: BTreeMap::new(),
            sequence: 1,
            active: None,
            fallback,
        }
    }
    fn terminate(&mut self, phase: Phase) {
        if let Some(active) = self.active.take() {
            (active.entry.sink)(Ingress::Update {
                serial: active.serial,
                address: active.entry.address.clone(),
                phase,
                position: active.last,
            });
        }
    }
    pub(super) fn cancel_dead(&mut self) {
        if self
            .active
            .as_ref()
            .is_some_and(|active| !active.entry.alive.load(Ordering::Acquire))
        {
            self.terminate(Phase::Cancel);
        }
    }
    /// OLE callbacks never call GetData. Only the legacy file path reads after the actual drop.
    pub(super) fn route(&mut self, event: drag::Event) -> bool {
        let (offer, point, phase) = match event {
            drag::Event::Leave => {
                self.terminate(Phase::Leave);
                return false;
            }
            drag::Event::Enter { offer, position } => {
                self.terminate(Phase::Leave);
                (offer, position, Phase::Enter)
            }
            drag::Event::Move { offer, position } => (offer, position, Phase::Move),
            drag::Event::Drop { offer, position } => (offer, position, Phase::Drop),
        };
        self.cancel_dead();
        let formats = offer.formats();
        let next = self
            .entries
            .values()
            .filter(|entry| entry.contains(point))
            .max_by(|left, right| {
                left.order
                    .cmp(&right.order)
                    .then_with(|| left.path.cmp(&right.path))
            })
            .and_then(|entry| entry.resolve(point))
            .filter(|entry| {
                entry
                    .preferred
                    .iter()
                    .any(|name| formats.contains(&name.as_str()))
            });
        let Some(next) = next else {
            self.terminate(Phase::Leave);
            if formats.contains(&transfer::FILES_MIME) {
                return phase != Phase::Drop
                    || self
                        .fallback
                        .try_send(fallback::Legacy {
                            offer,
                            position: point,
                        })
                        .is_ok();
            }
            return false;
        };
        if self.active.as_ref().is_none_or(|active| {
            !Arc::ptr_eq(&active.entry.alive, &next.alive)
                || active
                    .entry
                    .address
                    .canvas
                    .as_ref()
                    .map(|anchor| &anchor.target)
                    != next.address.canvas.as_ref().map(|anchor| &anchor.target)
        }) {
            self.terminate(Phase::Leave);
            static SERIAL: AtomicU64 = AtomicU64::new(1);
            let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
            let source = HostTransferOffer::new(
                Origin::OperatingSystem,
                formats
                    .into_iter()
                    .map(|mime| Format {
                        mime: mime.into(),
                        byte_len: None,
                    })
                    .collect(),
                Arc::new(Reader(offer)),
            );
            let Ok(source) = source else {
                return false;
            };
            let position = next.position(point);
            if !(next.sink)(Ingress::Enter {
                serial,
                address: next.address.clone(),
                expected: transfer::DropTarget {
                    preferred: next.preferred.clone(),
                },
                source: source.with_native_lifetime(next.alive.clone()),
                position,
            }) {
                return false;
            }
            self.active = Some(Active {
                serial,
                entry: next.clone(),
                last: position,
            });
        }
        if phase == Phase::Enter {
            return true;
        }
        let active = self.active.as_mut().unwrap();
        active.entry = next;
        active.last = active.entry.position(point);
        let accepted = (active.entry.sink)(Ingress::Update {
            serial: active.serial,
            address: active.entry.address.clone(),
            phase,
            position: active.last,
        });
        if phase == Phase::Drop || !accepted {
            self.active = None;
        }
        accepted
    }
}

/// Foreign apartment data is requested only by the runtime's admitted post-drop background reader.
struct Reader(Arc<drag::Offer>);
impl TransferReader for Reader {
    fn is_live(&self) -> bool {
        self.0.is_live()
    }
    fn read(&self, mime: &str) -> Result<HostTransferData, api::Failure> {
        self.0
            .read(mime, transfer::MAX_BYTES)
            .map(|value| match value {
                drag::Data::Bytes(bytes) => HostTransferData::Bytes(Item {
                    mime: mime.into(),
                    bytes,
                }),
                drag::Data::Files(paths) => HostTransferData::Files(paths),
            })
            .map_err(|error| api::Failure::new(api::ErrorCode::OperationFailed, error.to_string()))
    }
}
