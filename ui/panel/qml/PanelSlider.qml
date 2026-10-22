// SPDX-License-Identifier: GPL-2.0-or-later
// Vertical glass slider (macOS Control Center style): value label on top, a translucent track with a
// white fill with a flat top, the icon inside at the bottom. Drag, click, scroll or arrow keys.
import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Item {
    id: root

    property real s: 1
    property bool dark: true
    property color ink: "white"
    property real from: 0
    property real to: 100
    property real value: 0
    /// Snap to multiples of this (0 = continuous).
    property real stepSize: 0
    property string iconName
    property string text: Math.round(shown) + "%"
    property string accessibleName
    property bool available: true
    /// Touchpad panel mode: the finger is in this slider's column.
    property bool highlighted: false
    /// Maps a y in `item` to a coordinate that stays put on screen even when the panel resizes
    /// (the panel scales live while its size slider is dragged).
    property var toStable: function (item, y) { return item.mapToItem(null, 0, y).y; }

    signal moved(real value)
    signal released(real value)

    readonly property bool dragging: area.pressed
    property real dragValue: value
    property real pressStable: 0
    property real pressValue: 0
    property real pressHeight: 1
    readonly property real shown: dragging ? dragValue : value
    readonly property real fraction: to > from ? Math.max(0, Math.min(1, (shown - from) / (to - from))) : 0
    readonly property bool active: dragging || highlighted

    // the panel gives each slider a touchpad column; these are only defaults
    implicitWidth: Math.round(50 * s)
    implicitHeight: label.implicitHeight + column.spacing + Math.round(196 * s)
    opacity: available ? 1 : 0.35
    activeFocusOnTab: available

    Accessible.role: Accessible.Slider
    Accessible.name: accessibleName
    Accessible.description: text

    function snap(v) {
        if (stepSize > 0)
            v = from + Math.round((v - from) / stepSize) * stepSize;
        return Math.max(from, Math.min(to, v));
    }
    function setValue(v) {
        v = snap(v);
        if (v !== dragValue || !dragging) {
            dragValue = v;
            root.moved(v);
        }
    }
    // a press jumps to the touched point, dragging then follows the finger's movement
    function press(y) {
        setValue(from + (1 - Math.max(0, Math.min(1, y / track.height))) * (to - from));
        pressValue = dragValue;
        pressStable = toStable(area, y);
        pressHeight = Math.max(1, track.height);
    }
    function drag(y) {
        const up = pressStable - toStable(area, y);
        setValue(pressValue + up / pressHeight * (to - from));
    }
    function nudge(dir) {
        const step = stepSize > 0 ? stepSize : (to - from) / 20;
        const v = snap(shown + dir * step);
        dragValue = v;
        root.moved(v);
        root.released(v);
    }

    Keys.onUpPressed: nudge(1)
    Keys.onDownPressed: nudge(-1)

    Column {
        id: column
        width: root.width
        spacing: Math.round(9 * root.s)

        QQC2.Label {
            id: label
            anchors.horizontalCenter: parent.horizontalCenter
            text: root.text
            color: root.ink
            opacity: root.active ? 0.95 : 0.7
            font.pixelSize: Math.round(12.5 * root.s)
            font.weight: Font.DemiBold
            font.features: { "tnum": 1 } // digits do not jump while the value changes
            Behavior on opacity {
                NumberAnimation { duration: 150 }
            }
        }

        Item {
            id: track
            width: root.width
            height: Math.max(0, root.height - label.implicitHeight - column.spacing)
            readonly property real radius: Math.round(Math.min(width * 0.36, 20 * root.s))
            // the slider under the finger comes forward a little
            scale: root.active ? 1.03 : 1
            Behavior on scale {
                NumberAnimation { duration: 180; easing.type: Easing.OutCubic }
            }

            Rectangle {
                anchors.fill: parent
                radius: track.radius
                color: root.dark ? Qt.rgba(1, 1, 1, 0.10) : Qt.rgba(0, 0, 0, 0.07)
                border.width: 1
                border.color: root.dark ? Qt.rgba(1, 1, 1, 0.06) : Qt.rgba(0, 0, 0, 0.04)
            }

            // the fill: flat top edge, rounded like the track at the bottom; near the top its top
            // corners round off along the track's curve (per-corner radii, no shader needed)
            Rectangle {
                id: fill
                anchors.bottom: parent.bottom
                width: parent.width
                height: root.fraction * parent.height
                readonly property real gapTop: parent.height - height
                color: root.dark ? Qt.rgba(1, 1, 1, 0.92) : "white"
                bottomLeftRadius: Math.min(track.radius, height)
                bottomRightRadius: Math.min(track.radius, height)
                topLeftRadius: Math.max(0, track.radius - gapTop)
                topRightRadius: Math.max(0, track.radius - gapTop)
                Behavior on height {
                    // mouse drags follow the pointer exactly; values from the touchpad glide
                    enabled: !root.dragging
                    NumberAnimation { duration: root.highlighted ? 90 : 220; easing.type: Easing.OutCubic }
                }
            }

            // focus / touchpad column ring
            Rectangle {
                anchors.fill: parent
                anchors.margins: -Math.round(3 * root.s)
                radius: track.radius + Math.round(3 * root.s)
                color: "transparent"
                border.width: Math.max(2, Math.round(2 * root.s))
                border.color: root.highlighted ? Qt.rgba(1, 1, 1, root.dark ? 0.55 : 0.9) : Kirigami.Theme.focusColor
                opacity: root.highlighted || root.activeFocus ? 1 : 0
                Behavior on opacity {
                    NumberAnimation { duration: 150 }
                }
            }

            Kirigami.Icon {
                id: icon
                readonly property real bottomMargin: Math.round(13 * root.s)
                // dark once the white fill reaches it
                readonly property bool covered: fill.height >= bottomMargin + height * 0.55
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.bottom: parent.bottom
                anchors.bottomMargin: bottomMargin
                width: Math.round(22 * root.s)
                height: width
                source: root.iconName
                isMask: true
                color: covered ? Qt.rgba(0.11, 0.11, 0.12, 0.78) : root.ink
                Behavior on color {
                    ColorAnimation { duration: 120 }
                }
            }

            MouseArea {
                id: area
                anchors.fill: parent
                enabled: root.available
                preventStealing: true
                cursorShape: Qt.PointingHandCursor
                onPressed: mouse => {
                    root.forceActiveFocus();
                    root.dragValue = root.value;
                    root.press(mouse.y);
                }
                onPositionChanged: mouse => root.drag(mouse.y)
                onReleased: root.released(root.dragValue)
                onWheel: wheel => root.nudge(wheel.angleDelta.y > 0 ? 1 : -1)
            }
        }
    }
}
