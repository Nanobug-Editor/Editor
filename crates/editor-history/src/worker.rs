//! Bounded background ownership of blocking history, codec and retention work.

use crate::{Error, History, Result};
use futures::channel::oneshot;
use futures::{FutureExt, StreamExt};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Maximum queued or undelivered immutable payload, separate from on-disk capacity.
const RETAINED_LIMIT: usize = 64 * 1024 * 1024;
/// An already accepted operation executes even if its caller stops awaiting the reply.
type Job = Box<dyn FnOnce(&std::result::Result<History, String>) + Send>;

/// Native threads and host-managed executors share the same bounded submission contract.
enum Sender {
    /// Standalone library users retain the draining native worker lifecycle.
    Native(mpsc::SyncSender<Job>),
    /// The GUI's own executor also supports its deterministic scheduler without foreign-thread wakes.
    Executor(Mutex<futures::channel::mpsc::Sender<Job>>),
}

/// Short diagnostic snapshot; reading it never opens the database or waits for queued I/O.
#[derive(Clone, Debug, Default)]
pub struct WorkerStatus {
    /// Initialization completed, successfully or otherwise.
    pub initialized: bool,
    /// A usable store was opened; maintenance errors do not make this false.
    pub available: bool,
    /// Last initialization or maintenance failure for the host's non-repeating notification.
    pub error: Option<String>,
}

/// Cloneable background handle. The application retains one owner until shutdown.
#[derive(Clone)]
pub struct Worker(Arc<Inner>);

/// Dropping the final owner drains accepted work before joining; do this after closing the UI.
struct Inner {
    /// Bounded job count also limits metadata-only requests.
    sender: Option<Sender>,
    /// Byte reservations include replies that have not yet been consumed.
    retained: Arc<AtomicUsize>,
    /// Worker-owned lifecycle and maintenance diagnostics.
    status: Arc<Mutex<WorkerStatus>>,
    /// One serial owner avoids detached writes at application exit.
    thread: Option<JoinHandle<()>>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// A reservation travels with its reply, so cancellation cannot free bytes still held by a job.
struct Reservation(Arc<AtomicUsize>, usize);

impl Drop for Reservation {
    fn drop(&mut self) {
        self.0.fetch_sub(self.1, Ordering::AcqRel);
    }
}

impl Worker {
    /// Run initializer off the calling thread. Storage errors are returned by call and status.
    pub fn start(initializer: impl FnOnce() -> Result<History> + Send + 'static) -> Result<Self> {
        let (sender, receiver) = mpsc::sync_channel::<Job>(16);
        let status = Arc::new(Mutex::new(WorkerStatus::default()));
        let background_status = status.clone();
        let thread = std::thread::Builder::new()
            .name("editor-history".into())
            .spawn(move || {
                let history = initializer().map_err(|error| error.to_string());
                if let Ok(mut status) = background_status.lock() {
                    status.initialized = true;
                    status.available = history.is_ok();
                    status.error = history.as_ref().err().cloned();
                }
                let mut deadline = Instant::now() + Duration::from_secs(30);
                loop {
                    if Instant::now() >= deadline {
                        maintain(&history, &background_status);
                        deadline = Instant::now() + Duration::from_secs(30);
                    }
                    match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    {
                        Ok(job) => job(&history),
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                }
            })?;
        Ok(Self(Arc::new(Inner {
            sender: Some(Sender::Native(sender)),
            retained: Arc::new(AtomicUsize::new(0)),
            status,
            thread: Some(thread),
        })))
    }

    /// Run the same serialized, bounded worker on a host's background executor and timer.
    /// The host must await a call barrier before shutdown; dropping the handle closes accepted work.
    pub fn on_executor(
        initializer: impl FnOnce() -> Result<History> + Send + 'static,
        spawn: impl FnOnce(futures::future::BoxFuture<'static, ()>),
        timer: impl Fn(Duration) -> futures::future::BoxFuture<'static, ()> + Send + 'static,
    ) -> Self {
        let (sender, mut receiver) = futures::channel::mpsc::channel::<Job>(15);
        let status = Arc::new(Mutex::new(WorkerStatus::default()));
        let background_status = status.clone();
        spawn(Box::pin(async move {
            let history = initializer().map_err(|error| error.to_string());
            if let Ok(mut status) = background_status.lock() {
                status.initialized = true;
                status.available = history.is_ok();
                status.error = history.as_ref().err().cloned();
            }
            // Requests must not restart the maintenance period. Poll its ready deadline first
            // so a continuously occupied queue cannot starve expiration or physical compaction.
            let mut timeout = timer(Duration::from_secs(30)).fuse();
            loop {
                let next = receiver.next().fuse();
                futures::pin_mut!(next);
                futures::select_biased! {
                    _ = timeout => {
                        maintain(&history, &background_status);
                        timeout = timer(Duration::from_secs(30)).fuse();
                    },
                    job = next => match job { Some(job) => job(&history), None => break },
                }
            }
        }));
        Self(Arc::new(Inner {
            sender: Some(Sender::Executor(Mutex::new(sender))),
            retained: Arc::new(AtomicUsize::new(0)),
            status,
            thread: None,
        }))
    }

    /// Submit action without blocking the caller. retained_bytes bounds captured input plus reply.
    /// Queue exhaustion, initialization, storage and disconnected-worker errors remain distinct.
    pub fn call<T: Send + 'static, F: FnOnce(&History) -> Result<T> + Send + 'static>(
        &self,
        retained_bytes: usize,
        action: F,
    ) -> impl std::future::Future<Output = Result<T>> + Send + 'static + use<T, F> {
        let submitted = self.submit(retained_bytes, action);
        async move {
            let (result, _reservation) = submitted?.await.map_err(|_| Error::Unavailable)?;
            result
        }
    }

    /// Enqueue immediately, so a successful disk save is recorded even before its UI reply is polled.
    fn submit<T: Send + 'static>(
        &self,
        retained_bytes: usize,
        action: impl FnOnce(&History) -> Result<T> + Send + 'static,
    ) -> Result<oneshot::Receiver<(Result<T>, Reservation)>> {
        self.0
            .retained
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current
                    .checked_add(retained_bytes)
                    .filter(|total| *total <= RETAINED_LIMIT)
            })
            .map_err(|_| Error::Busy)?;
        let reservation = Reservation(self.0.retained.clone(), retained_bytes);
        let (send, receive) = oneshot::channel();
        let job: Job = Box::new(move |history| {
            let result = match history {
                Ok(history) => action(history),
                Err(error) => Err(Error::Initialization(error.clone())),
            };
            let _ = send.send((result, reservation));
        });
        match self.0.sender.as_ref().ok_or(Error::Unavailable)? {
            Sender::Native(sender) => sender.try_send(job).map_err(|error| match error {
                mpsc::TrySendError::Full(_) => Error::Busy,
                mpsc::TrySendError::Disconnected(_) => Error::Unavailable,
            })?,
            Sender::Executor(sender) => sender
                .lock()
                .map_err(|_| Error::Unavailable)?
                .try_send(job)
                .map_err(|error| {
                    if error.is_full() {
                        Error::Busy
                    } else {
                        Error::Unavailable
                    }
                })?,
        }
        Ok(receive)
    }

    /// Return the last background diagnostic without performing storage work.
    pub fn status(&self) -> WorkerStatus {
        self.0
            .status
            .lock()
            .map(|status| status.clone())
            .unwrap_or(WorkerStatus {
                initialized: true,
                available: false,
                error: Some(Error::Unavailable.to_string()),
            })
    }
}

/// Periodic retention shares the worker's serialized owner with captures and queries.
fn maintain(history: &std::result::Result<History, String>, status: &Mutex<WorkerStatus>) {
    if let Ok(history) = history {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(i64::MAX as u128) as i64;
        let result = history.maintain(now);
        if let Ok(mut status) = status.lock() {
            status.error = result.err().map(|error| error.to_string());
        }
    }
}
