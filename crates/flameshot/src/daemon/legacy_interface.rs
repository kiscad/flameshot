// SPDX-License-Identifier: GPL-3.0-or-later
//! Preserve Qt's method annotations while retaining zbus-generated dispatch.
//! zbus 5.19's interface macro has no arbitrary method-annotation attribute.
use super::Service;
use std::{collections::HashMap, fmt::Write};
use zbus::{
    Connection, ObjectServer, fdo,
    message::{Header, Message},
    names::{InterfaceName, MemberName},
    object_server::{DispatchResult2, Interface, SignalEmitter},
    zvariant::{OwnedValue, Value},
};

pub(super) struct LegacyService(pub(super) Service);

#[zbus::export::async_trait::async_trait]
impl Interface for LegacyService {
    fn name() -> InterfaceName<'static> {
        Service::name()
    }
    fn spawn_tasks_for_methods(&self) -> bool {
        self.0.spawn_tasks_for_methods()
    }
    async fn get(
        &self,
        property_name: &str,
        server: &ObjectServer,
        connection: &Connection,
        header: Option<&Header<'_>>,
        emitter: &SignalEmitter<'_>,
    ) -> Option<fdo::Result<OwnedValue>> {
        self.0
            .get(property_name, server, connection, header, emitter)
            .await
    }
    async fn get_all(
        &self,
        server: &ObjectServer,
        connection: &Connection,
        header: Option<&Header<'_>>,
        emitter: &SignalEmitter<'_>,
    ) -> fdo::Result<HashMap<String, OwnedValue>> {
        self.0.get_all(server, connection, header, emitter).await
    }
    fn set<'call>(
        &'call self,
        property_name: &'call str,
        value: &'call Value<'_>,
        server: &'call ObjectServer,
        connection: &'call Connection,
        header: Option<&'call Header<'_>>,
        emitter: &'call SignalEmitter<'_>,
    ) -> DispatchResult2<'call> {
        self.0
            .set(property_name, value, server, connection, header, emitter)
    }
    async fn set_mut(
        &mut self,
        property_name: &str,
        value: &Value<'_>,
        server: &ObjectServer,
        connection: &Connection,
        header: Option<&Header<'_>>,
        emitter: &SignalEmitter<'_>,
    ) -> Option<fdo::Result<()>> {
        self.0
            .set_mut(property_name, value, server, connection, header, emitter)
            .await
    }
    fn call<'call>(
        &'call self,
        server: &'call ObjectServer,
        connection: &'call Connection,
        message: &'call Message,
        name: MemberName<'call>,
    ) -> DispatchResult2<'call> {
        self.0.call(server, connection, message, name)
    }
    fn call_mut<'call>(
        &'call mut self,
        server: &'call ObjectServer,
        connection: &'call Connection,
        message: &'call Message,
        name: MemberName<'call>,
    ) -> DispatchResult2<'call> {
        self.0.call_mut(server, connection, message, name)
    }
    fn introspect_to_writer(&self, writer: &mut dyn Write, level: usize) {
        let mut generated = String::new();
        self.0.introspect_to_writer(&mut generated, level);
        let mut no_reply = false;
        for line in generated.lines() {
            match line.trim() {
                "<method name=\"captureScreen\">"
                | "<method name=\"attachScreenshotToClipboard\">"
                | "<method name=\"attachTextToClipboard\">"
                | "<method name=\"attachPin\">" => no_reply = true,
                "</method>" => {
                    if no_reply {
                        let indent = &line[..line.len() - line.trim_start().len()];
                        let _ = writeln!(
                            writer,
                            "{indent}  <annotation name=\"org.freedesktop.DBus.Method.NoReply\" value=\"true\"/>"
                        );
                    }
                    no_reply = false;
                }
                _ => {}
            }
            let _ = writeln!(writer, "{line}");
        }
    }
}
