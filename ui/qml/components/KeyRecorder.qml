// SPDX-License-Identifier: GPL-2.0-or-later
import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.kaget

// Button that records the next key combination. `mode` "key" produces Linux
// key codes (KEY_LEFTCTRL+KEY_T), "shortcut" a human shortcut (Ctrl+T).
QQC2.Button {
    id: recorder

    property string mode: "key"
    property bool recording: false
    property string partial: ""

    signal recorded(string value)

    text: recording ? (partial !== "" ? partial + "+…" : i18nc("@action:button", "Press keys…")) : i18nc("@action:button", "Record")
    icon.name: recording ? "media-playback-stop" : "media-record"
    checkable: true
    checked: recording
    focusPolicy: Qt.StrongFocus

    onClicked: {
        recording = !recording;
        partial = "";
        if (recording)
            forceActiveFocus();
    }
    onActiveFocusChanged: {
        if (!activeFocus)
            recording = false;
    }

    function convert(key: int, modifiers: int): string {
        return mode === "key" ? Helpers.keyComboFromEvent(key, modifiers) : Helpers.shortcutFromEvent(key, modifiers);
    }

    Keys.onPressed: (event) => {
        if (!recording)
            return;
        event.accepted = true;
        if (Helpers.isModifierKey(event.key)) {
            partial = mode === "key" ? Helpers.prettyKey(convert(event.key, event.modifiers)) : convert(event.key, event.modifiers);
            return;
        }
        const value = convert(event.key, event.modifiers);
        recording = false;
        partial = "";
        if (value !== "")
            recorded(value);
    }
    Keys.onReleased: (event) => {
        if (recording)
            event.accepted = true;
    }
}
