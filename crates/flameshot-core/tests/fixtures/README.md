# Qt wire fixtures

Generated locally using Qt 6.4.2, its default QDataStream version and big-endian
byte order. Images are synthetic solid-color data, not desktop screenshots.

- `qt6-clipboard.bin`: QDataStream << QPixmap from a 3x2 RGBA8888 QImage,
  filled with QColor(20,80,160,255). Pixmap devicePixelRatio was 1.5 (Qt does not
  serialize that ratio inside the pixmap itself).
- `qt6-pin.bin`: the same pixmap followed by QRect(-20,30,3,2) and qreal(1.5).

The stream contains a big-endian 32-bit marker 1, a complete PNG through IEND,
then (for pins) four big-endian signed QRect coordinates x1,y1,x2,y2, followed by
an optional big-endian IEEE754 double. The rectangle uses inclusive right/bottom.

The generation and reverse-check executables were compiled under ignored
`target/qt-fixtures` and run with QT_QPA_PLATFORM=offscreen. Qt development headers
were extracted there; the Rust project and these tests do not depend on Qt.
The Rust `qt_wire_roundtrip` example re-encodes a pin fixture to a chosen output
path for external interoperability checks.
