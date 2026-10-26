//! Real OLE data crosses STA/MTA through the native adapter; only selection invokes its producer.
use platform_windows::drag;
pub(super) mod legacy;
mod os;
pub(super) mod outgoing;
pub(super) use os::pointer;
pub(super) use os::{prepare as prepare_os, prepare_node, run as run_os};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

/// An external delayed renderer makes native GetData calls observable without mocking host policy.
struct Producer(Arc<AtomicUsize>);
impl drag::Provider for Producer {
    fn formats(&self) -> Vec<String> {
        vec!["text/plain;charset=utf-8".into(), "text/html".into()]
    }
    fn read(&self, mime: &str) -> Result<drag::Data, drag::Error> {
        self.0.fetch_add(1, Ordering::AcqRel);
        Ok(drag::Data::Bytes(match mime {
            "text/html" => "<table><tr><td>中文</td></tr></table><script>inert()</script>"
                .as_bytes()
                .to_vec(),
            _ => "中文\t42".as_bytes().to_vec(),
        }))
    }
}

/// Capture an actual COM IDataObject on GPUI's STA without invoking its delayed renderer.
pub(super) fn prepare() -> anyhow::Result<(Arc<drag::Offer>, Arc<AtomicUsize>)> {
    let count = Arc::new(AtomicUsize::new(0));
    let source = drag::data_object(Arc::new(Producer(count.clone())))?;
    let offer = drag::Offer::capture(&source)?;
    anyhow::ensure!(offer.formats() == ["text/plain;charset=utf-8", "text/html"]);
    anyhow::ensure!(
        count.load(Ordering::Acquire) == 0,
        "OLE enumeration must not render data"
    );
    Ok((offer, count))
}

/// Resolve one selected format from a worker apartment while GPUI continues pumping the source STA.
pub(super) fn read((offer, count): (Arc<drag::Offer>, Arc<AtomicUsize>)) -> anyhow::Result<()> {
    let drag::Data::Bytes(bytes) = offer.read("text/html", 256 * 1024)? else {
        anyhow::bail!("selected HTML must remain inert bytes")
    };
    anyhow::ensure!(
        bytes == "<table><tr><td>中文</td></tr></table><script>inert()</script>".as_bytes()
    );
    anyhow::ensure!(
        count.load(Ordering::Acquire) == 1,
        "only the chosen representation may render"
    );
    offer.revoke();
    anyhow::ensure!(offer.read("text/plain;charset=utf-8", 256 * 1024).is_err());
    anyhow::ensure!(
        count.load(Ordering::Acquire) == 1,
        "revocation must not call a late producer"
    );
    Ok(())
}
