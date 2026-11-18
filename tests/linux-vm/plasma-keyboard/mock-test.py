"""Adapts plasma-keyboard's mock-compositor test to the check.

The test enables the layout `PK_LAYOUT` names instead of `en`, and gains
`testShiftLevel`, which presses the key `PK_SHIFT_RIGHTS` navigation steps
in, then taps the first key twice and prints what each tap committed.
"""

import sys

path = sys.argv[1]
source = open(path).read()

enabled = 'QStringLiteral("org.kde.plasma.keyboard.en/mobile-qwerty")'
assert source.count(enabled) == 1, "the test no longer enables en itself"
source = source.replace(
    enabled,
    'qEnvironmentVariable("PK_LAYOUT", QStringLiteral("org.kde.plasma.keyboard.en/mobile-qwerty"))',
)

test = r'''
    void testShiftLevel()
    {
        if (!m_inputPanel->surface()) {
            QSignalSpy surfaceSpy(m_inputPanel.get(), &InputPanelV1::inputPanelSurfaceCreated);
            QVERIFY(surfaceSpy.wait());
        }
        auto *surface = m_inputPanel->surface();
        QVERIFY(surface);
        QTRY_VERIFY_WITH_TIMEOUT(surface->hasContent(), 5000);
        const int rights = qEnvironmentVariableIntValue("PK_SHIFT_RIGHTS");
        for (int i = 0; i < rights; ++i) {
            sendKey(KEY_RIGHT, 20);
        }
        sendKey(KEY_ENTER, 100);
        sendKey(KEY_ESC, 100);
        QTest::qWait(500);
        const QSize surfaceSize = inputPanelSurfaceSize();
        const qreal keyboardHeight = std::max(surfaceSize.height() * 0.3, 150.0);
        const QPointF first(surfaceSize.width() / 20.0, surfaceSize.height() - keyboardHeight + keyboardHeight / 10);
        for (const char *label : {"SHIFTED", "UNSHIFTED"}) {
            QSignalSpy commitStringSpy(m_inputMethod->context(), &InputMethodContext::commitStringChanged);
            tapInputPanel(first);
            QVERIFY(commitStringSpy.count() || commitStringSpy.wait());
            qInfo().noquote() << label << commitStringSpy.first().first().toString();
            QTest::qWait(300);
        }
    }

'''
anchor = "\n    // Verify navigation consumes its keys and Escape restores forwarding\n"
assert source.count(anchor) == 1, "the test's navigation test moved"
source = source.replace(anchor, "\n" + test + anchor.lstrip("\n"), 1)
open(path, "w").write(source)
