// SPDX-License-Identifier: GPL-2.0-or-later
import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.kaget

// One draggable line of the touchpad map: an area line (`area.<edge>`) or a
// side-drag line (`side_drag.<edge>`). Dragging edits the configuration copy;
// the value is clamped so that the centre keeps >= 50 % of width and height.
Item {
    id: line

    required property Item map
    required property Item pad
    required property string section
    required property string edge
    property real gripPosition: 0.5

    readonly property string path: section + "." + edge
    readonly property bool isArea: section === "area"
    readonly property bool horizontal: edge === "top" || edge === "bottom"
    readonly property real valueMm: map.cfg[section]?.[edge] ?? 0
    readonly property real maxMm: map.maxFor(section, edge)
    readonly property color color: isArea ? map.areaColor : map.sideColor
    readonly property real hit: Math.max(8, Kirigami.Units.gridUnit * 0.6)
    readonly property real s: map.s
    readonly property real pos: {
        switch (edge) {
        case "top": return valueMm * s;
        case "bottom": return pad.height - valueMm * s;
        case "left": return valueMm * s;
        default: return pad.width - valueMm * s;
        }
    }
    readonly property bool active: mouse.containsMouse || mouse.pressed || map.forcedReadout === path
    readonly property string edgeLabel: {
        switch (edge) {
        case "top": return isArea ? i18nc("@label line name", "Top area") : i18nc("@label line name", "Top side drag");
        case "right": return isArea ? i18nc("@label line name", "Right area") : i18nc("@label line name", "Right side drag");
        case "bottom": return isArea ? i18nc("@label line name", "Bottom area") : i18nc("@label line name", "Bottom side drag");
        default: return isArea ? i18nc("@label line name", "Left area") : i18nc("@label line name", "Left side drag");
        }
    }

    z: active ? 40 : (isArea ? 20 : 21)
    x: horizontal ? 0 : pos - hit / 2
    y: horizontal ? pos - hit / 2 : 0
    width: horizontal ? pad.width : hit
    height: horizontal ? hit : pad.height

    // The line itself.
    Rectangle {
        anchors.centerIn: parent
        width: line.horizontal ? parent.width : (line.active ? 3 : 2)
        height: line.horizontal ? (line.active ? 3 : 2) : parent.height
        color: line.color
        opacity: line.valueMm > 0 ? (line.isArea ? 0.95 : 0.85) : 0.35
    }

    // Grip, the visible affordance for dragging.
    Rectangle {
        visible: line.map.interactive
        readonly property real len: Kirigami.Units.gridUnit * 1.3
        width: line.horizontal ? len : 7
        height: line.horizontal ? 7 : len
        radius: 3.5
        x: line.horizontal ? Math.max(0, Math.min(line.width - width, line.width * line.gripPosition - width / 2)) : (line.width - width) / 2
        y: line.horizontal ? (line.height - height) / 2 : Math.max(0, Math.min(line.height - height, line.height * line.gripPosition - height / 2))
        color: line.color
        border.width: 1
        border.color: Kirigami.Theme.backgroundColor
        scale: line.active ? 1.25 : 1
        Behavior on scale { NumberAnimation { duration: Kirigami.Units.shortDuration } }
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        enabled: line.map.interactive
        hoverEnabled: true
        preventStealing: true
        cursorShape: line.horizontal ? Qt.SizeVerCursor : Qt.SizeHorCursor

        function update(mouseX: real, mouseY: real, modifiers: int) {
            const p = mapToItem(line.pad, mouseX, mouseY);
            let mm;
            switch (line.edge) {
            case "top": mm = p.y / line.s; break;
            case "bottom": mm = (line.pad.height - p.y) / line.s; break;
            case "left": mm = p.x / line.s; break;
            default: mm = (line.pad.width - p.x) / line.s; break;
            }
            // 0.5 mm steps, Shift for 0.1 mm.
            const step = (modifiers & Qt.ShiftModifier) ? 0.1 : 0.5;
            mm = Math.round(mm / step) * step;
            mm = Math.max(0, Math.min(line.maxMm, mm));
            mm = Math.round(mm * 10) / 10;
            if (Math.abs(mm - line.valueMm) > 0.0001)
                DaemonClient.setDraftValue(line.path, mm);
        }

        onPressed: (ev) => update(ev.x, ev.y, ev.modifiers)
        onPositionChanged: (ev) => {
            if (pressed)
                update(ev.x, ev.y, ev.modifiers);
        }
    }

    // Live mm readout next to the grip.
    Rectangle {
        id: readout
        visible: line.active
        z: 50
        readonly property real gripX: line.horizontal ? line.width * line.gripPosition : line.width / 2
        readonly property real gripY: line.horizontal ? line.height / 2 : line.height * line.gripPosition
        readonly property real gap: Kirigami.Units.largeSpacing
        width: readoutLabel.implicitWidth + Kirigami.Units.largeSpacing
        height: readoutLabel.implicitHeight + Kirigami.Units.smallSpacing * 2
        radius: Kirigami.Units.cornerRadius
        color: Kirigami.Theme.backgroundColor
        border.width: 1
        border.color: line.color
        x: {
            if (line.horizontal)
                return Math.max(-line.x, Math.min(line.pad.width - line.x - width, gripX - width / 2));
            return line.edge === "left" ? gripX + gap : gripX - gap - width;
        }
        y: {
            if (!line.horizontal)
                return Math.max(-line.y, Math.min(line.pad.height - line.y - height, gripY - height / 2));
            return line.edge === "top" ? gripY + gap : gripY - gap - height;
        }

        QQC2.Label {
            id: readoutLabel
            anchors.centerIn: parent
            text: i18nc("@info %1 line name, %2 distance in mm, %3 maximum", "%1: %2 mm (max %3)",
                        line.edgeLabel, line.valueMm.toLocaleString(Qt.locale(), "f", 1), line.maxMm.toLocaleString(Qt.locale(), "f", 1))
            font: Kirigami.Theme.smallFont
        }
    }
}
