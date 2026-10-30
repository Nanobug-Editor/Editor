//! Completion wakeups preserve mailbox order and never keep an idle actor spinning.
use super::*;

/// A reply arriving before or during receive must end the idle wait without a synthetic Work.
#[test]
fn native_completion_wakes_waiting_actor() {
    let (sender, receiver) = channel("native-completion", Default::default());
    let (started, ready) = mpsc::channel();
    let (finished, result) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        started.send(()).unwrap();
        finished
            .send(matches!(
                receiver.recv_timeout(Duration::from_secs(30)),
                Err(mpsc::RecvTimeoutError::Timeout)
            ))
            .unwrap();
    });
    ready.recv().unwrap();
    sender.wake();
    assert!(result.recv_timeout(Duration::from_secs(2)).unwrap());
    thread.join().unwrap();
}

/// Wakeups neither reorder admitted input nor occupy its bounded capacity, and consume once.
#[test]
fn native_completion_wakes_coalesce_and_preserve_fifo() {
    let (sender, receiver) = channel("native-completion-fifo", Default::default());
    for index in 0..256 {
        sender.send(Work::Disable(index.to_string())).unwrap();
    }
    for _ in 0..1000 {
        sender.wake();
    }
    for index in 0..256 {
        assert!(matches!(
            receiver.recv_timeout(Duration::ZERO).unwrap(),
            Work::Disable(id) if id == index.to_string()
        ));
    }
    sender.wake();
    sender.wake();
    assert!(matches!(
        receiver.recv_timeout(Duration::from_secs(30)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    // The consumed wake must not turn the next idle receive into an immediate poll.
    let (finished, result) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        let disconnected = matches!(
            receiver.recv_timeout(Duration::from_secs(30)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        );
        finished.send(disconnected).unwrap();
    });
    assert!(result.recv_timeout(Duration::from_millis(20)).is_err());
    drop(sender);
    assert!(result.recv_timeout(Duration::from_secs(2)).unwrap());
    thread.join().unwrap();
}
