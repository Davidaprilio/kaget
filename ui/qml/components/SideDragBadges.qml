// SPDX-License-Identifier: GPL-2.0-or-later
import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.kaget

// Finger badges ("3F ↓ Overview") for one side-drag band of the touchpad map.
Item {
    id: badges

    // Above the draggable lines; the line grips sit away from the badges.
    z: 30

    required property string edge
    required property Item map

    readonly property bool horizontal: edge === "top" || edge === "bottom"
    readonly property real depthMm: edge === "top" ? map.sideTop : edge === "right" ? map.sideRight : edge === "bottom" ? map.sideBottom : map.sideLeft
    readonly property real depth: depthMm * map.s
    readonly property real areaMm: edge === "top" ? map.areaTop : edge === "right" ? map.areaRight : edge === "bottom" ? map.areaBottom : map.areaLeft
    readonly property string arrow: edge === "top" ? "↓" : edge === "bottom" ? "↑" : edge === "left" ? "→" : "←"
    readonly property real pillHeight: Math.round(Kirigami.Units.gridUnit * 1.05)
    readonly property real pillWidth: Math.round(Kirigami.Units.gridUnit * 2.3)
    readonly property real maxLabelWidth: horizontal
        ? Math.max(0, (map.padW - 2 * map.cornerW * map.s) / Math.max(1, map.sideDragFingers.length) - pillWidth - Kirigami.Units.gridUnit * 1.5)
        : Math.max(Kirigami.Units.gridUnit * 2, Math.max(areaMm, 14) * map.s - pillInset - Kirigami.Units.smallSpacing - Kirigami.Units.largeSpacing)
    // Distance from the pad edge to the inner edge of a pill (left/right edges).
    readonly property real pillInset: Math.max(2 + pillWidth, depth / 2 + pillWidth / 2)
    readonly property bool showText: map.roomy && !map.showNumberPad

    Grid {
        id: grid
        flow: badges.horizontal ? Grid.LeftToRight : Grid.TopToBottom
        columns: badges.horizontal ? Math.max(1, badges.map.sideDragFingers.length) : 1
        spacing: badges.horizontal ? Kirigami.Units.gridUnit : Kirigami.Units.smallSpacing
        horizontalItemAlignment: badges.edge === "right" ? Grid.AlignRight : Grid.AlignLeft
        x: {
            switch (badges.edge) {
            case "top":
            case "bottom":
                return (badges.map.padW - width) / 2;
            case "left":
                return Math.max(2, badges.depth / 2 - badges.pillWidth / 2);
            default:
                return Math.min(badges.map.padW - 2, badges.map.padW - badges.depth / 2 + badges.pillWidth / 2) - width;
            }
        }
        y: {
            switch (badges.edge) {
            case "top":
                return badges.depth / 2 - badges.pillHeight / 2;
            case "bottom":
                return badges.map.padH - badges.depth / 2 - badges.pillHeight / 2;
            default:
                return (badges.map.padH - height) / 2;
            }
        }

        Repeater {
            model: badges.map.sideDragFingers
            delegate: Item {
                id: badge
                required property int modelData
                readonly property string trigger: "sidedrag." + badges.edge + "." + modelData
                readonly property string action: badges.map.actionOf(trigger)
                readonly property var info: (DaemonClient.actions, DaemonClient.actionInfo(action))
                readonly property bool previewed: badges.map.previewTrigger === trigger && badges.map.selectedZone === "side." + badges.edge
                width: badgeRow.implicitWidth
                height: badgeRow.implicitHeight

                Row {
                id: badgeRow
                spacing: Kirigami.Units.smallSpacing
                layoutDirection: badges.edge === "right" ? Qt.RightToLeft : Qt.LeftToRight

                Rectangle {
                    width: badges.pillWidth
                    height: badges.pillHeight
                    radius: height / 2
                    color: badge.action !== "" ? badges.map.sideColor : Kirigami.ColorUtils.linearInterpolation(badges.map.sideColor, Kirigami.Theme.backgroundColor, 0.55)
                    border.width: badge.previewed ? 2 : 0
                    border.color: Kirigami.Theme.focusColor
                    Row {
                        anchors.centerIn: parent
                        spacing: 2
                        QQC2.Label {
                            text: i18nc("@label finger count badge, e.g. 3F", "%1F", badge.modelData)
                            color: "white"
                            font.pixelSize: Math.round(badges.pillHeight * 0.62)
                            font.weight: Font.Bold
                        }
                        QQC2.Label {
                            // One glyph rotated per edge, so all arrows look alike.
                            text: "↓"
                            color: "white"
                            font.pixelSize: Math.round(badges.pillHeight * 0.62)
                            font.weight: Font.Bold
                            rotation: badges.edge === "top" ? 0 : badges.edge === "bottom" ? 180 : badges.edge === "left" ? -90 : 90
                        }
                    }
                }
                QQC2.Label {
                    visible: badges.showText && badges.maxLabelWidth > Kirigami.Units.gridUnit
                    anchors.verticalCenter: parent.verticalCenter
                    width: Math.min(implicitWidth, badges.maxLabelWidth)
                    text: badge.action !== "" ? badge.info.label : "—"
                    font: Kirigami.Theme.smallFont
                    elide: Text.ElideRight
                    opacity: badge.action !== "" ? 1 : 0.5
                    horizontalAlignment: badges.edge === "right" ? Text.AlignRight : Text.AlignLeft
                }

                }

                MouseArea {
                    anchors.fill: parent
                    enabled: badges.map.interactive
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        badges.map.zoneActivated("side." + badges.edge);
                        badges.map.triggerActivated(badge.trigger);
                    }
                }
            }
        }
    }
}
