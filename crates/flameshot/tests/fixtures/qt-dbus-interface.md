# Qt D-Bus interface reference

`qt-dbus-interface.xml` contains the `org.flameshot.Flameshot` interface exported
by the legacy adaptor using Qt 6.4.2 (Ubuntu package
`6.4.2+dfsg-21.1build5`). It is an observed reference, not XML reconstructed from
the Rust service.

The declaration used was `src/core/flameshotdbusadapter.h`, unchanged from migration
baseline `f4f2e4e8`. Its SHA-256 is
`7ad85af5230c1d6b08ee9560dd434a919ccfabb002655c54e680e93a9526781b`.
To regenerate:

1. Retrieve that header from the baseline and run Qt's `moc` on it.
2. Build a QCoreApplication harness with the generated moc source. Define the adaptor
   constructor using `QDBusAbstractAdaptor(parent)`, its destructor, and empty bodies
   for the four slots. No capture, clipboard or other application code is needed to
   inspect metadata.
3. Attach the adaptor to a QObject, register the object at `/` with
   `QDBusConnection::ExportAdaptors`, and own `org.flameshot.Reference` on a private
   session bus. Run the application's event loop.
4. Run `gdbus introspect --session --dest org.flameshot.Reference --object-path / --xml`
   on that private bus. Extract the `org.flameshot.Flameshot` interface and normalize
   whitespace only. Stop the harness and private bus.

The fixture intentionally excludes Qt's generic D-Bus interfaces. The daemon test
compares all methods, argument attributes/order and method annotations structurally,
rejecting duplicates and unexpected members. It also tests standard Properties
behavior separately. Qt is only needed to regenerate the reference; Rust builds and
normal tests use the checked-in XML and have no Qt dependency.
