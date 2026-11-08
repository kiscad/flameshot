# SPDX-License-Identifier: GPL-3.0-or-later
"""Physical function-key input for private Xvfb fixtures.

Some xdotool/XKB combinations choose an Alt level for repeated F-key symbols.
Send the resolved keycodes through XTEST, with only the explicitly requested
modifiers. Callers must pass the display created by their own fixture.
"""
import ctypes
import re
import time


def press(display, chords, delay_ms=12):
    assert display and all(re.fullmatch(r'(?:Super_L\+)?F(?:[1-9]|1[0-2])', chord) for chord in chords)
    x11 = ctypes.CDLL('libX11.so.6')
    xtest = ctypes.CDLL('libXtst.so.6')
    x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
    x11.XOpenDisplay.restype = ctypes.c_void_p
    x11.XStringToKeysym.argtypes = [ctypes.c_char_p]
    x11.XStringToKeysym.restype = ctypes.c_ulong
    x11.XKeysymToKeycode.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
    x11.XKeysymToKeycode.restype = ctypes.c_ubyte
    x11.XSync.argtypes = [ctypes.c_void_p, ctypes.c_int]
    x11.XCloseDisplay.argtypes = [ctypes.c_void_p]
    xtest.XTestFakeKeyEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
    connection = x11.XOpenDisplay(display.encode())
    assert connection, 'Cannot open private test display'
    held = []
    try:
        for chord in chords:
            for key in chord.split('+'):
                code = x11.XKeysymToKeycode(connection, x11.XStringToKeysym(key.encode()))
                assert code, f'No keycode for {key}'
                assert xtest.XTestFakeKeyEvent(connection, code, True, 0)
                held.append(code)
            x11.XSync(connection, False)
            time.sleep(delay_ms / 1000)
            for code in reversed(held):
                assert xtest.XTestFakeKeyEvent(connection, code, False, 0)
            held.clear()
            x11.XSync(connection, False)
            time.sleep(delay_ms / 1000)
    finally:
        for code in reversed(held):
            xtest.XTestFakeKeyEvent(connection, code, False, 0)
        x11.XSync(connection, False)
        x11.XCloseDisplay(connection)
