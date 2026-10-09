//! Public ureq transport extension retains the actual TCP cancellation handle underneath TLS and CONNECT.
use super::*;
use std::io::{Read, Write};
use ureq::unversioned::{
    resolver::{DefaultResolver, Resolver},
    transport::{
        Buffers, ConnectionDetails, Connector, Either, LazyBuffers, NextTimeout, Transport,
        time::Duration as TransportDuration,
    },
};

#[derive(Debug)]
pub(super) struct OwnedConnector(pub(super) Arc<Control>);
impl<In: Transport> Connector<In> for OwnedConnector {
    type Out = Either<In, Socket>;
    fn connect(
        &self,
        details: &ConnectionDetails,
        chained: Option<In>,
    ) -> Result<Option<Self::Out>, ureq::Error> {
        if let Some(transport) = chained {
            return Ok(Some(Either::A(transport)));
        }
        self.0
            .check()
            .map_err(|error| ureq::Error::Other(Box::new(error)))?;
        let mut last = None;
        for address in &details.addrs {
            self.0
                .check()
                .map_err(|error| ureq::Error::Other(Box::new(error)))?;
            let timeout = self
                .0
                .deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(250));
            if timeout.is_zero() {
                return Err(ureq::Error::Timeout(ureq::Timeout::Global));
            }
            match TcpStream::connect_timeout(address, timeout) {
                Ok(stream) => {
                    let mut owned = self.0.socket.lock().unwrap();
                    self.0
                        .check()
                        .map_err(|error| ureq::Error::Other(Box::new(error)))?;
                    // Retain the actual same native socket, including Windows pending IO, underneath wrappers.
                    let stream = Arc::new(stream);
                    *owned = Some(stream.clone());
                    stream.set_nodelay(true)?;
                    return Ok(Some(Either::B(Socket {
                        stream,
                        buffers: LazyBuffers::new(
                            details.config.input_buffer_size(),
                            details.config.output_buffer_size(),
                        ),
                        control: self.0.clone(),
                    })));
                }
                Err(error) => last = Some(error),
            }
        }
        Err(last
            .map(ureq::Error::Io)
            .unwrap_or(ureq::Error::ConnectionFailed))
    }
}
pub(super) struct Socket {
    stream: Arc<TcpStream>,
    buffers: LazyBuffers,
    control: Arc<Control>,
}
impl std::fmt::Debug for Socket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OwnedHttpSocket")
    }
}
impl Socket {
    fn timeout(&self, next: NextTimeout) -> Result<Duration, ureq::Error> {
        self.control
            .check()
            .map_err(|error| ureq::Error::Other(Box::new(error)))?;
        let remaining = self
            .control
            .deadline
            .saturating_duration_since(Instant::now());
        Ok(if next.after.is_not_happening() {
            remaining
        } else {
            remaining.min(*next.after)
        }
        .max(Duration::from_millis(1)))
    }
}
impl Transport for Socket {
    fn buffers(&mut self) -> &mut dyn Buffers {
        &mut self.buffers
    }
    fn transmit_output(&mut self, amount: usize, timeout: NextTimeout) -> Result<(), ureq::Error> {
        let until = Instant::now() + self.timeout(timeout)?;
        let mut sent = 0;
        // A short socket poll is retried inside the transport; it is never a renewed request deadline.
        while sent < amount {
            self.control
                .check()
                .map_err(|error| ureq::Error::Other(Box::new(error)))?;
            let remaining = until.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(ureq::Error::Timeout(timeout.reason));
            }
            self.stream
                .set_write_timeout(Some(remaining.min(Duration::from_millis(50))))?;
            match (&*self.stream).write(&self.buffers.output()[sent..amount]) {
                Ok(0) => return Err(std::io::Error::from(std::io::ErrorKind::WriteZero).into()),
                Ok(count) => sent += count,
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                    ) => {}
                Err(error) => return Err(error.into()),
            }
        }
        self.control
            .check()
            .map_err(|error| ureq::Error::Other(Box::new(error)))
    }
    fn await_input(&mut self, timeout: NextTimeout) -> Result<bool, ureq::Error> {
        let until = Instant::now() + self.timeout(timeout)?;
        loop {
            self.control
                .check()
                .map_err(|error| ureq::Error::Other(Box::new(error)))?;
            let remaining = until.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(ureq::Error::Timeout(timeout.reason));
            }
            // Shutdown interrupts most sockets immediately; bounded polls cover pending Windows/TLS IO.
            self.stream
                .set_read_timeout(Some(remaining.min(Duration::from_millis(50))))?;
            match (&*self.stream).read(self.buffers.input_append_buf()) {
                Ok(count) => {
                    self.control
                        .check()
                        .map_err(|error| ureq::Error::Other(Box::new(error)))?;
                    self.buffers.input_appended(count);
                    return Ok(count > 0);
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                    ) => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
    fn is_open(&mut self) -> bool {
        self.control.check().is_ok()
    }
}

/// OS DNS is synchronous but lives inside the four counted producer slots, with checks before/after.
#[derive(Debug)]
pub(super) struct BoundedResolver(pub(super) Arc<Control>);
impl Resolver for BoundedResolver {
    fn resolve(
        &self,
        uri: &ureq::http::Uri,
        config: &ureq::config::Config,
        timeout: NextTimeout,
    ) -> Result<ureq::unversioned::resolver::ResolvedSocketAddrs, ureq::Error> {
        self.0
            .check()
            .map_err(|error| ureq::Error::Other(Box::new(error)))?;
        let result = DefaultResolver::default().resolve(
            uri,
            config,
            NextTimeout {
                after: TransportDuration::NotHappening,
                reason: timeout.reason,
            },
        );
        self.0
            .check()
            .map_err(|error| ureq::Error::Other(Box::new(error)))?;
        result
    }
}
