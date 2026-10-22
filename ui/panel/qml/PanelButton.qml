// SPDX-License-Identifier: GPL-2.0-or-later
// A panel slot that runs an action: same glass track as the sliders, the action's name where a slider
// shows its value, a large icon inside. Click it, or tap its touchpad column in panel mode.
import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Item {
    id: root

    property real s: 1
    property bool dark: true
    property color ink: "white"
    property string text
    property string iconName
    /// An application icon: keeps its own colours (theme action icons follow the text colour).
    property bool colorful: false
    /// Touchpad panel mode: the finger is in this button's column.
    property bool highlighted: false

    signal clicked()

    /// Brief white flash when the action runs.
    function flash() {
        flashAnimation.restart();
    }

    implicitWidth: Math.round(50 * s)
    implicitHeight: label.implicitHeight + column.spacing + Math.round(196 * s)
    activeFocusOnTab: true

    Accessible.role: Accessible.Button
    Accessible.name: text
    Accessible.onPressAction: root.clicked()
    Keys.onReturnPressed: root.clicked()
    Keys.onSpacePressed: root.clicked()

    Column {
        id: column
        width: root.width
        spacing: Math.round(9 * root.s)

        QQC2.Label {
            id: label
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideRight
            text: root.text
            color: root.ink
            opacity: root.highlighted || area.pressed ? 0.95 : 0.7
            font.pixelSize: Math.round(12.5 * root.s)
            font.weight: Font.DemiBold
        }

        Item {
            id: track
            width: root.width
            height: Math.max(0, root.height - label.implicitHeight - column.spacing)
            readonly property real radius: Math.round(Math.min(width * 0.36, 20 * root.s))
            scale: area.pressed ? 0.96 : (root.highlighted ? 1.03 : 1)
            Behavior on scale {
                NumberAnimation { duration: 160; easing.type: Easing.OutCubic }
            }

            Rectangle {
                anchors.fill: parent
                radius: track.radius
                color: area.containsMouse ? (root.dark ? Qt.rgba(1, 1, 1, 0.16) : Qt.rgba(0, 0, 0, 0.11))
                                          : (root.dark ? Qt.rgba(1, 1, 1, 0.10) : Qt.rgba(0, 0, 0, 0.07))
                border.width: 1
                border.color: root.dark ? Qt.rgba(1, 1, 1, 0.06) : Qt.rgba(0, 0, 0, 0.04)
                Behavior on color {
                    ColorAnimation { duration: 120 }
                }
            }

            // flash: the button lights up white like a full slider, then fades back
            Rectangle {
                id: flashFill
                anchors.fill: parent
                radius: track.radius
                color: root.dark ? Qt.rgba(1, 1, 1, 0.92) : "white"
                opacity: 0
                SequentialAnimation {
                    id: flashAnimation
                    NumberAnimation { target: flashFill; property: "opacity"; to: 1; duration: 70; easing.type: Easing.OutCubic }
                    NumberAnimation { target: flashFill; property: "opacity"; to: 0; duration: 380; easing.type: Easing.InOutQuad }
                }
            }

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
                anchors.centerIn: parent
                width: Math.round(Math.min(34 * root.s, track.width * 0.5))
                height: width
                source: root.iconName
                isMask: !root.colorful
                color: flashFill.opacity > 0.5 ? Qt.rgba(0.11, 0.11, 0.12, 0.78) : root.ink
            }

            MouseArea {
                id: area
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: {
                    root.forceActiveFocus();
                    root.clicked();
                }
            }
        }
    }
}
