// SPDX-License-Identifier: GPL-3.0-or-later
//! Clipboard providers whose lease ends only after ownership/active transfers end.
use anyhow::{Context, Result, ensure};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

pub struct Lease {
    active: Arc<AtomicBool>,
}
impl Lease {
    pub fn active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }
}
struct Active(Arc<AtomicBool>);
impl Drop for Active {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

pub fn copy(data: Vec<u8>, mime: &str, wayland: bool) -> Result<Lease> {
    ensure!(
        matches!(
            mime,
            "image/png" | "image/jpeg" | "text/plain;charset=utf-8"
        ),
        "Unsupported clipboard MIME type"
    );
    let data: Arc<[u8]> = data.into();
    if wayland {
        match start(data.clone(), mime.to_owned(), true) {
            Ok(lease) => return Ok(lease),
            Err(error) if std::env::var_os("DISPLAY").is_none() => return Err(error),
            Err(_) => {}
        }
    }
    start(data, mime.to_owned(), false)
}
fn start(data: Arc<[u8]>, mime: String, wayland: bool) -> Result<Lease> {
    let active = Arc::new(AtomicBool::new(false));
    let state = active.clone();
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _guard = Active(state.clone());
        let result = if wayland {
            serve_wayland(&data, &mime, &state, &sender)
        } else {
            serve_x11(&data, &mime, &state, &sender)
        };
        if let Err(error) = result {
            let _ = sender.send(Err(format!("{error:#}")));
        }
    });
    receiver
        .recv_timeout(Duration::from_secs(5))
        .context("Clipboard provider did not become ready")?
        .map_err(anyhow::Error::msg)?;
    Ok(Lease { active })
}
fn serve_wayland(
    data: &[u8],
    mime: &str,
    active: &AtomicBool,
    ready: &mpsc::Sender<Result<(), String>>,
) -> Result<()> {
    use wl_clipboard_rs::copy::{MimeType, Options, Source};
    let mut options = Options::new();
    options.foreground(true);
    let kind = if mime.starts_with("text/") {
        MimeType::Text
    } else {
        MimeType::Specific(mime.into())
    };
    let prepared = options.prepare_copy(Source::Bytes(data.into()), kind)?;
    active.store(true, Ordering::Release);
    if ready.send(Ok(())).is_err() {
        return Ok(());
    }
    prepared.serve()?;
    Ok(())
}
fn serve_x11(
    data: &[u8],
    mime: &str,
    active: &AtomicBool,
    ready: &mpsc::Sender<Result<(), String>>,
) -> Result<()> {
    use x11rb::{
        connection::{Connection, RequestConnection},
        protocol::{Event, xproto::*},
        wrapper::ConnectionExt as _,
    };
    let (connection, screen) = x11rb::connect(None).context("Cannot connect to X11 clipboard")?;
    let window = connection.generate_id()?;
    connection
        .create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            window,
            connection.setup().roots[screen].root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )?
        .check()?;
    let atom = |name: &str| -> Result<u32> {
        Ok(connection
            .intern_atom(false, name.as_bytes())?
            .reply()?
            .atom)
    };
    let clipboard = atom("CLIPBOARD")?;
    let targets = atom("TARGETS")?;
    let timestamp = atom("TIMESTAMP")?;
    let incr = atom("INCR")?;
    let mut formats = vec![atom(mime)?];
    if mime.starts_with("text/") {
        formats.push(atom("UTF8_STRING")?);
        formats.push(atom("text/plain")?);
    }
    connection.change_property8(
        PropMode::REPLACE,
        window,
        timestamp,
        AtomEnum::INTEGER,
        &[0],
    )?;
    connection.flush()?;
    let time = loop {
        if let Event::PropertyNotify(event) = connection.wait_for_event()?
            && event.window == window
            && event.atom == timestamp
        {
            break event.time;
        }
    };
    connection
        .set_selection_owner(window, clipboard, time)?
        .check()?;
    ensure!(
        connection.get_selection_owner(clipboard)?.reply()?.owner == window,
        "Clipboard ownership was refused"
    );
    active.store(true, Ordering::Release);
    if ready.send(Ok(())).is_err() {
        return Ok(());
    }
    struct Transfer {
        offset: usize,
        target: u32,
        last: Instant,
    }
    let mut transfers: HashMap<(u32, u32), Transfer> = HashMap::new();
    let mut owned = true;
    let chunk = (connection.maximum_request_bytes().saturating_sub(128)).min(32768);
    ensure!(chunk > 0, "X11 request limit is too small");
    while owned || !transfers.is_empty() {
        transfers.retain(|_, transfer| transfer.last.elapsed() < Duration::from_secs(10));
        let event = if transfers.is_empty() {
            if !owned {
                break;
            }
            Some(connection.wait_for_event()?)
        } else {
            connection.poll_for_event()?
        };
        let Some(event) = event else {
            std::thread::sleep(Duration::from_millis(2));
            continue;
        };
        match event {
            Event::SelectionClear(event) if event.selection == clipboard => owned = false,
            Event::SelectionRequest(request) if request.selection == clipboard => {
                let property = if request.property == 0 {
                    request.target
                } else {
                    request.property
                };
                let mut supported = true;
                if request.target == targets {
                    let mut list = vec![targets, timestamp];
                    list.extend(&formats);
                    connection.change_property32(
                        PropMode::REPLACE,
                        request.requestor,
                        property,
                        AtomEnum::ATOM,
                        &list,
                    )?;
                } else if request.target == timestamp {
                    connection.change_property32(
                        PropMode::REPLACE,
                        request.requestor,
                        property,
                        AtomEnum::INTEGER,
                        &[time],
                    )?;
                } else if formats.contains(&request.target) {
                    if data.len() <= chunk {
                        connection.change_property8(
                            PropMode::REPLACE,
                            request.requestor,
                            property,
                            request.target,
                            data,
                        )?;
                    } else if transfers.len() < 64 {
                        connection.change_window_attributes(
                            request.requestor,
                            &ChangeWindowAttributesAux::new().event_mask(
                                EventMask::PROPERTY_CHANGE | EventMask::STRUCTURE_NOTIFY,
                            ),
                        )?;
                        connection.change_property32(
                            PropMode::REPLACE,
                            request.requestor,
                            property,
                            incr,
                            &[u32::try_from(data.len())?],
                        )?;
                        transfers.insert(
                            (request.requestor, property),
                            Transfer {
                                offset: 0,
                                target: request.target,
                                last: Instant::now(),
                            },
                        );
                    } else {
                        supported = false;
                    }
                } else {
                    supported = false;
                }
                connection.send_event(
                    false,
                    request.requestor,
                    EventMask::NO_EVENT,
                    SelectionNotifyEvent {
                        response_type: SELECTION_NOTIFY_EVENT,
                        sequence: 0,
                        time: request.time,
                        requestor: request.requestor,
                        selection: clipboard,
                        target: request.target,
                        property: if supported { property } else { 0 },
                    },
                )?;
                connection.flush()?;
            }
            Event::PropertyNotify(event) if event.state == Property::DELETE => {
                let key = (event.window, event.atom);
                if let Some(transfer) = transfers.get_mut(&key) {
                    let end = (transfer.offset + chunk).min(data.len());
                    connection.change_property8(
                        PropMode::REPLACE,
                        event.window,
                        event.atom,
                        transfer.target,
                        &data[transfer.offset..end],
                    )?;
                    let done = transfer.offset == data.len();
                    transfer.offset = end;
                    transfer.last = Instant::now();
                    if done {
                        transfers.remove(&key);
                        if !transfers.keys().any(|(window, _)| *window == event.window) {
                            connection.change_window_attributes(
                                event.window,
                                &ChangeWindowAttributesAux::new().event_mask(EventMask::NO_EVENT),
                            )?;
                        }
                    }
                    connection.flush()?;
                }
            }
            Event::DestroyNotify(event) => {
                transfers.retain(|(window, _), _| *window != event.window)
            }
            _ => {}
        }
    }
    Ok(())
}
