// SPDX-License-Identifier: GPL-3.0-or-later
use std::sync::mpsc::{Receiver, TryRecvError};

/// Only an empty live channel is pending. A dropped worker is a terminal error,
/// while a buffered result takes precedence even if the sender has already exited.
pub fn poll<T>(receiver: &Receiver<Result<T, String>>, stopped: &str) -> Option<Result<T, String>> {
    match receiver.try_recv() {
        Ok(result) => Some(result),
        Err(TryRecvError::Empty) => None,
        Err(TryRecvError::Disconnected) => Some(Err(stopped.into())),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pending_cancelled_failed_and_disconnected_are_distinct() {
        let (sender, receiver) = std::sync::mpsc::channel::<Result<Option<u8>, String>>();
        assert_eq!(poll(&receiver, "stopped"), None);
        sender.send(Ok(None)).unwrap();
        sender.send(Err("request failed".into())).unwrap();
        sender.send(Ok(Some(7))).unwrap();
        drop(sender);
        assert_eq!(poll(&receiver, "stopped"), Some(Ok(None)));
        assert_eq!(
            poll(&receiver, "stopped"),
            Some(Err("request failed".into()))
        );
        assert_eq!(poll(&receiver, "stopped"), Some(Ok(Some(7))));
        assert_eq!(poll(&receiver, "stopped"), Some(Err("stopped".into())));
    }
}
