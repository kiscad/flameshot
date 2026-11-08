# Linux public D-Bus compatibility

Service and interface: `org.flameshot.Flameshot`. Object path: `/`.
All four methods have no output arguments and publish
`org.freedesktop.DBus.Method.NoReply=true`, matching the checked-in Qt introspection
reference. Calls that request a reply still receive an empty success reply; clients
using the no-reply flag still trigger the operation.

| Method | Input signature | Verified behavior |
| --- | --- | --- |
| captureScreen | empty | Opens a native capture window; reply and no-reply calls work; closing the child leaves the daemon alive |
| attachPin | ay | Accepts the real Qt pin fixture and creates a native pin for both call modes; malformed data returns InvalidArgs |
| attachScreenshotToClipboard | ay | Decodes QDataStream pixmap bytes; normal calls preserve exact image pixels; a no-reply call with the real Qt fixture replaces the X11 clipboard with its exact pixels |
| attachTextToClipboard | ss | Transfers text and optional notification; normal/no-reply calls work; queued text/image replacements remain ordered even when notification handling is held |

`crates/flameshot/tests/daemon.rs` compares runtime introspection with both the Qt
reference (`tests/fixtures/qt-dbus-interface.xml` within the application crate) and
the installed resource source (`data/dbus/org.flameshot.Flameshot.xml`). Comparison
includes method/argument names, types, direction, order and annotations. The test
also verifies empty Properties/GetAll, unknown-property errors and single ownership
when another daemon starts. Staging separately compares the installed XML contract.

`crates/flameshot-core/src/qt_wire.rs` tests the actual Qt-produced clipboard/pin
fixtures, exact pixels, negative position, geometry, fractional scale, omitted legacy
scale, round trips and rejected truncation/corruption/invalid scale. Routine Rust
builds and tests do not use Qt. Fixture provenance is retained with the fixtures.

Native protocol tests run through `tests/rust/tray_smoke.py` on a private Xvfb display
and D-Bus. They wait for the previous editor to close before issuing the next public
request, then wait for each new child to appear and close. The wrapper requires the
named Rust test and its passing summary, because Flameshot's legacy CLI parse-error
status is zero and is not evidence that a test harness ran. A negative check supplying
the application executable confirms that this misuse is rejected.

Reproduction with installed test utilities:

```sh
python3 tests/rust/dbus_ui_smoke.py /
```

This builds the correct test executables and additionally exercises private portal
and pin-clipboard integration. The public-method native test passes after the latest
coverage additions; the broader wrapper suite also passed during this audit.
The minimal-feature private-bus test and all-feature test Clippy check also pass.
The audit covers the Linux public protocol above. Installed sandbox policy, broader
compositor acceptance and hosted CI remain in their respective migration gates.
