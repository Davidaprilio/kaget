// SPDX-License-Identifier: GPL-2.0-or-later
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import io.github.kaget

// Binding editor for one zone of the touchpad map (corner, side-drag band,
// area band or centre): lists its triggers with action pickers.
ColumnLayout {
    id: editor

    property string zone: ""
    property string previewTrigger: ""
    property bool showHeading: true
    property int fingers: 3

    signal previewRequested(string trigger)

    readonly property var cfg: DaemonClient.draft
    readonly property int maxFingers: DaemonClient.state.device?.max_fingers ?? 5
    readonly property var triggerList: triggersFor(zone, fingers, DaemonClient.triggers)

    spacing: Kirigami.Units.smallSpacing

    function triggersFor(zone, fingers, all) {
        const names = all.map(t => t.trigger);
        const match = (fn) => names.filter(fn);
        if (zone.startsWith("corner.")) {
            const prefix = zone + ".";
            return match(t => t.startsWith(prefix));
        }
        if (zone.startsWith("side.")) {
            const prefix = "sidedrag." + zone.substring(5) + ".";
            return match(t => t.startsWith(prefix));
        }
        if (zone.startsWith("band.")) {
            const edge = zone.substring(5);
            return match(t => {
                const p = t.split(".");
                if (p[0] !== "corner" || p[2] !== "swipe")
                    return false;
                const horizontal = p[3] === "left" || p[3] === "right";
                const vertical = p[3] === "up" || p[3] === "down";
                if (edge === "top")
                    return horizontal;
                if (edge === "right")
                    return vertical && p[1].endsWith("right");
                if (edge === "left")
                    return vertical && p[1].endsWith("left");
                return false;
            });
        }
        if (zone === "centre") {
            return match(t => {
                const p = t.split(".");
                if (["swipe", "pinch", "rotate"].includes(p[0]))
                    return parseInt(p[1]) === fingers;
                if (["hold", "tap", "doubletap", "taphold"].includes(p[0]))
                    return parseInt(p[1]) === fingers;
                return false;
            });
        }
        return [];
    }

    function describe(zone) {
        const mm = (v) => Number(v ?? 0).toLocaleString(Qt.locale(), "f", 1);
        if (zone.startsWith("corner."))
            return i18n("%1 × %2 mm. Hold still for %3 ms, or start here and swipe more than 7 mm.",
                        mm(cfg.corners?.width), mm(cfg.corners?.height), cfg.corners?.hold_ms ?? 0);
        if (zone.startsWith("side.")) {
            const edge = zone.substring(5);
            return i18n("%1 mm deep. Land all fingers inside the band and drag inwards at least %2 mm.",
                        mm(cfg.side_drag?.[edge]), mm(cfg.side_drag?.min_travel_mm));
        }
        if (zone.startsWith("band.")) {
            const edge = zone.substring(5);
            const depth = cfg.area?.[edge] ?? 0;
            if (depth <= 0)
                return i18n("This area is disabled (0 mm). Drag its line inwards to enable it.");
            if (edge === "bottom")
                return i18n("%1 mm deep. No v1 gesture continues in the bottom area.", mm(depth));
            return i18n("%1 mm deep. After a slider gesture from a corner ends, swiping along the same axis in this area within %2 s continues it. These corner swipes use it:",
                        mm(depth), Number(cfg.slider?.continue_s ?? 0).toLocaleString(Qt.locale(), "f", 1));
        }
        if (zone === "centre")
            return i18n("Gestures with three or more fingers anywhere on the touchpad. Two-finger gestures always stay with the system.");
        return "";
    }

    Kirigami.Heading {
        visible: editor.showHeading
        text: DaemonClient.zoneTitle(editor.zone)
        level: 3
        wrapMode: Text.Wrap
        Layout.fillWidth: true
    }

    QQC2.Label {
        text: editor.describe(editor.zone)
        wrapMode: Text.Wrap
        opacity: 0.75
        Layout.fillWidth: true
        Layout.bottomMargin: Kirigami.Units.smallSpacing
    }

    RowLayout {
        visible: editor.zone === "centre"
        spacing: 0
        Layout.fillWidth: true
        QQC2.ButtonGroup {
            id: fingerGroup
        }
        Repeater {
            model: Math.max(0, editor.maxFingers - 2)
            delegate: QQC2.Button {
                required property int index
                readonly property int count: index + 3
                text: i18np("%1 finger", "%1 fingers", count)
                checkable: true
                checked: editor.fingers === count
                QQC2.ButtonGroup.group: fingerGroup
                onClicked: editor.fingers = count
                Layout.fillWidth: true
            }
        }
    }

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        visible: editor.zone === "centre" && editor.fingers <= 4 && !(editor.cfg.general?.override_system_gestures ?? false)
        type: Kirigami.MessageType.Information
        text: DaemonClient.passive ? i18n("Rows marked “System” are used by KDE. Rebinding them requires filter mode.")
                                   : i18n("Rows marked “System” belong to KWin or libinput. Enable “Override system gestures” on the Gestures page to rebind them.")
    }

    Repeater {
        model: editor.triggerList
        delegate: TriggerRow {
            required property string modelData
            info: (DaemonClient.triggers, DaemonClient.triggerInfo(modelData))
            compact: true
            selectable: true
            selected: editor.previewTrigger === modelData
            onPreviewRequested: editor.previewRequested(modelData)
            Layout.fillWidth: true
        }
    }

    QQC2.Label {
        visible: editor.triggerList.length === 0 && editor.zone !== "" && editor.zone !== "band.bottom"
        text: DaemonClient.triggers.length === 0 ? i18n("The gesture list is not loaded yet.") : i18n("No gestures use this area.")
        opacity: 0.7
        wrapMode: Text.Wrap
        Layout.fillWidth: true
    }

    QQC2.Label {
        visible: editor.triggerList.length > 0
        text: i18n("Click a row to preview it on the map.")
        font: Kirigami.Theme.smallFont
        opacity: 0.6
        wrapMode: Text.Wrap
        Layout.fillWidth: true
        Layout.topMargin: Kirigami.Units.smallSpacing
    }
}
