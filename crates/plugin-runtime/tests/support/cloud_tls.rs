//! Ephemeral test certificates stay in memory and never modify the OS trust store.
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub struct Certificates {
    pub root: Vec<u8>,
    pub server: Arc<rustls::ServerConfig>,
}
impl Certificates {
    /// .NET CertificateRequest generates an independent short-lived localhost chain without extra tools.
    pub fn new() -> Self {
        let script = r#"
$ErrorActionPreference = 'Stop'
$caKey = [Security.Cryptography.RSA]::Create(2048)
$leafKey = [Security.Cryptography.RSA]::Create(2048)
$hash = [Security.Cryptography.HashAlgorithmName]::SHA256
$padding = [Security.Cryptography.RSASignaturePadding]::Pkcs1
$caRequest = [Security.Cryptography.X509Certificates.CertificateRequest]::new('CN=Nanobug ephemeral QA CA', $caKey, $hash, $padding)
$caRequest.CertificateExtensions.Add([Security.Cryptography.X509Certificates.X509BasicConstraintsExtension]::new($true, $false, 0, $true))
$caRequest.CertificateExtensions.Add([Security.Cryptography.X509Certificates.X509KeyUsageExtension]::new([Security.Cryptography.X509Certificates.X509KeyUsageFlags]::KeyCertSign, $true))
$now = [DateTimeOffset]::UtcNow
$ca = $caRequest.CreateSelfSigned($now.AddMinutes(-5), $now.AddHours(1))
$leafRequest = [Security.Cryptography.X509Certificates.CertificateRequest]::new('CN=localhost', $leafKey, $hash, $padding)
$leafRequest.CertificateExtensions.Add([Security.Cryptography.X509Certificates.X509BasicConstraintsExtension]::new($false, $false, 0, $true))
$leafRequest.CertificateExtensions.Add([Security.Cryptography.X509Certificates.X509KeyUsageExtension]::new([Security.Cryptography.X509Certificates.X509KeyUsageFlags]::DigitalSignature -bor [Security.Cryptography.X509Certificates.X509KeyUsageFlags]::KeyEncipherment, $true))
$oids = [Security.Cryptography.OidCollection]::new()
$null = $oids.Add([Security.Cryptography.Oid]::new('1.3.6.1.5.5.7.3.1'))
$leafRequest.CertificateExtensions.Add([Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension]::new($oids, $true))
$san = [Security.Cryptography.X509Certificates.SubjectAlternativeNameBuilder]::new()
$san.AddDnsName('localhost')
$leafRequest.CertificateExtensions.Add($san.Build())
$leaf = $leafRequest.Create($ca, $now.AddMinutes(-5), $now.AddMinutes(50), [Security.Cryptography.RandomNumberGenerator]::GetBytes(16))
@{root=[Convert]::ToBase64String($ca.RawData); cert=[Convert]::ToBase64String($leaf.RawData); key=[Convert]::ToBase64String($leafKey.ExportPkcs8PrivateKey())} | ConvertTo-Json -Compress
$leaf.Dispose(); $ca.Dispose(); $leafKey.Dispose(); $caKey.Dispose()
"#;
        let result = std::process::Command::new("pwsh")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                script,
            ])
            .output()
            .expect("Existing PowerShell 7 is required for the ephemeral TLS fixture");
        assert!(
            result.status.success(),
            "Ephemeral certificate generation failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let result: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        // Decode only test-generated material; neither the JSON nor the private key is logged.
        use base64::Engine;
        let bytes = |key: &str| {
            base64::engine::general_purpose::STANDARD
                .decode(result[key].as_str().unwrap())
                .unwrap()
        };
        let configuration = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![bytes("cert").into()],
            rustls::pki_types::PrivateKeyDer::Pkcs8(bytes("key").into()),
        )
        .unwrap();
        Self {
            root: bytes("root"),
            server: Arc::new(configuration),
        }
    }
}

/// A real HTTP CONNECT tunnel records only its cleartext proxy request and owns every fixture socket.
pub struct Proxy {
    pub url: String,
    pub requests: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    sockets: Arc<Mutex<Vec<TcpStream>>>,
    thread: Option<JoinHandle<()>>,
}
impl Proxy {
    pub fn new(target: &str) -> Self {
        let port = target.rsplit(':').next().unwrap().parse::<u16>().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://localhost:{}", listener.local_addr().unwrap().port());
        let stop = Arc::new(AtomicBool::new(false));
        let sockets = Arc::new(Mutex::new(Vec::<TcpStream>::new()));
        let requests = Arc::new(Mutex::new(vec![]));
        let (stopped, owned, received) = (stop.clone(), sockets.clone(), requests.clone());
        let thread = thread::spawn(move || {
            let mut clients = vec![];
            while !stopped.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut client, _)) => {
                        // Windows propagates listener nonblocking mode to accepted sockets.
                        client.set_nonblocking(false).unwrap();
                        client
                            .set_read_timeout(Some(Duration::from_secs(3)))
                            .unwrap();
                        client
                            .set_write_timeout(Some(Duration::from_secs(3)))
                            .unwrap();
                        owned.lock().unwrap().push(client.try_clone().unwrap());
                        let (owned, received) = (owned.clone(), received.clone());
                        clients.push(thread::spawn(move || {
                            let mut headers = vec![];
                            let mut byte = [0];
                            while headers.len() < 32768 && !headers.ends_with(b"\r\n\r\n") {
                                if client.read(&mut byte).unwrap_or(0) == 0 {
                                    return;
                                }
                                headers.push(byte[0]);
                            }
                            let headers = String::from_utf8_lossy(&headers).into_owned();
                            received.lock().unwrap().push(headers.clone());
                            if !headers.starts_with("CONNECT ") {
                                return;
                            }
                            let Ok(mut target) = TcpStream::connect(("127.0.0.1", port)) else {
                                return;
                            };
                            owned.lock().unwrap().push(target.try_clone().unwrap());
                            target
                                .set_read_timeout(Some(Duration::from_secs(3)))
                                .unwrap();
                            target
                                .set_write_timeout(Some(Duration::from_secs(3)))
                                .unwrap();
                            if client
                                .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
                                .is_err()
                            {
                                return;
                            }
                            let (mut input, mut output) =
                                (client.try_clone().unwrap(), target.try_clone().unwrap());
                            let upstream = thread::spawn(move || {
                                let _ = std::io::copy(&mut input, &mut output);
                                let _ = output.shutdown(std::net::Shutdown::Write);
                            });
                            let _ = std::io::copy(&mut target, &mut client);
                            let _ = client.shutdown(std::net::Shutdown::Both);
                            let _ = target.shutdown(std::net::Shutdown::Both);
                            upstream.join().unwrap();
                        }));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(_) => break,
                }
            }
            for client in clients {
                client.join().unwrap();
            }
        });
        Self {
            url,
            requests,
            stop,
            sockets,
            thread: Some(thread),
        }
    }
}
impl Drop for Proxy {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        for stream in self.sockets.lock().unwrap().iter() {
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}
