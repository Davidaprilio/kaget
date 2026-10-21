// SPDX-License-Identifier: GPL-2.0-or-later
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import io.github.kaget

Kirigami.Page {
    id: page

    title: i18nc("@title", "Touchpad Map")
    footer: ApplyBar {}
    padding: 0

    property string selectedZone: ""
    property string previewTrigger: ""
    property bool numberPadOverlay: false
    property bool demoHover: false
    readonly property bool sidePanel: width >= Kirigami.Units.gridUnit * 44
    readonly property bool numberpadSupported: DaemonClient.state.numberpad?.supported ?? false
    readonly property var cfg: DaemonClient.draft

    function selectZone(zone: string) {
        selectedZone = zone;
        const list = sideEditor.triggersFor(zone, sideEditor.fingers, DaemonClient.triggers);
        let pick = list.find(t => DaemonClient.actionFor(t) !== "");
        previewTrigger = pick ?? (list.length > 0 ? list[0] : "");
        if (!sidePanel)
            zoneDialog.open();
    }

    actions: [
        Kirigami.Action {
            text: i18nc("@action:intoolbar", "Live view")
            icon.name: "media-record"
            checkable: true
            checked: DaemonClient.live
            enabled: DaemonClient.connected
            tooltip: i18n("Show your fingers on the map in real time")
            onToggled: DaemonClient.setLive(checked)
        },
        Kirigami.Action {
            text: i18nc("@action:intoolbar", "NumberPad keys")
            icon.name: "accessories-calculator"
            checkable: true
            visible: page.numberpadSupported
            checked: page.numberPadOverlay
            onToggled: page.numberPadOverlay = checked
        }
    ]

    RowLayout {
        anchors.fill: parent
        spacing: 0

        QQC2.ScrollView {
            id: mainScroll
            Layout.fillWidth: true
            Layout.fillHeight: true
            // contentWidth follows the width, so do not let it feed back into the layout.
            Layout.preferredWidth: 1
            Layout.minimumWidth: 0
            contentWidth: availableWidth
            QQC2.ScrollBar.horizontal.policy: QQC2.ScrollBar.AlwaysOff

            ColumnLayout {
                id: mainColumn
                width: mainScroll.availableWidth
                spacing: Kirigami.Units.largeSpacing

                // Legend
                Flow {
                    Layout.fillWidth: true
                    Layout.topMargin: Kirigami.Units.largeSpacing
                    Layout.leftMargin: Kirigami.Units.largeSpacing
                    Layout.rightMargin: Kirigami.Units.largeSpacing
                    spacing: Kirigami.Units.largeSpacing

                    Repeater {
                        model: [
                            {label: i18n("Gesture area lines"), color: tpMap.areaColor, kind: "line", path: "area.enabled", fallback: true},
                            {label: i18n("Side-drag lines"), color: tpMap.sideColor, kind: "line", path: "side_drag.enabled", fallback: false},
                            {label: i18n("Corners"), color: tpMap.cornerColor, kind: "box", path: "corners.enabled", fallback: true},
                            {label: i18n("Finger preview"), color: tpMap.fingerColor, kind: "dot"}
                        ]
                        delegate: Row {
                            required property var modelData
                            readonly property bool toggle: (modelData.path ?? "") !== ""
                            readonly property bool on: !toggle || (DaemonClient.draft, DaemonClient.draftValue(modelData.path, modelData.fallback) === true)
                            spacing: Kirigami.Units.smallSpacing
                            opacity: on ? 1 : 0.55
                            QQC2.CheckBox {
                                visible: parent.toggle
                                anchors.verticalCenter: parent.verticalCenter
                                checked: parent.on
                                onToggled: {
                                    DaemonClient.setDraftValue(modelData.path, checked);
                                    checked = Qt.binding(() => parent.on);
                                }
                                QQC2.ToolTip.text: checked ? i18n("Click to switch this feature off") : i18n("Click to switch this feature on")
                                QQC2.ToolTip.visible: hovered
                                QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                            }
                            Rectangle {
                                anchors.verticalCenter: parent.verticalCenter
                                width: modelData.kind === "line" ? Kirigami.Units.gridUnit : Kirigami.Units.gridUnit * 0.7
                                height: modelData.kind === "line" ? 3 : width
                                radius: modelData.kind === "dot" ? width / 2 : 1
                                color: modelData.kind === "box" ? Qt.rgba(modelData.color.r, modelData.color.g, modelData.color.b, 0.35) : modelData.color
                                border.width: modelData.kind === "box" ? 1 : 0
                                border.color: modelData.color
                            }
                            QQC2.Label {
                                text: modelData.label
                                font: Kirigami.Theme.smallFont
                            }
                        }
                    }
                }

                // The map itself
                Item {
                    id: mapHolder
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.largeSpacing
                    Layout.rightMargin: Kirigami.Units.largeSpacing
                    Layout.preferredHeight: tpMap.height
                    readonly property real reserved: Kirigami.Units.gridUnit * (page.sidePanel ? 12 : 9)

                    TouchpadMap {
                        id: tpMap
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: Math.min(mapHolder.width, Math.max(Kirigami.Units.gridUnit * 14, (page.height - mapHolder.reserved) * aspect))
                        height: width / aspect
                        selectedZone: page.selectedZone
                        previewTrigger: page.previewTrigger
                        showNumberPad: page.numberPadOverlay || (DaemonClient.live && (DaemonClient.state.numberpad?.active ?? false))
                        live: DaemonClient.live
                        freezePreview: applicationWindow().screenshotMode
                        forcedReadout: page.demoHover ? "area.top" : ""
                        onZoneActivated: (zone) => page.selectZone(zone)
                        onTriggerActivated: (trigger) => page.previewTrigger = trigger
                    }
                }

                QQC2.Label {
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.largeSpacing
                    Layout.rightMargin: Kirigami.Units.largeSpacing
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.Wrap
                    font: Kirigami.Theme.smallFont
                    opacity: 0.7
                    visible: !DaemonClient.live
                    text: page.previewTrigger !== ""
                        ? i18nc("@info %1 gesture name", "Preview: %1", (DaemonClient.triggers, DaemonClient.triggerInfo(page.previewTrigger).longTitle))
                        : i18n("Drag a line to resize its area (hold Shift for 0.1 mm steps). Click a corner, side or area to edit its gestures.")
                }

                // Live status
                RowLayout {
                    visible: DaemonClient.live
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.largeSpacing
                    Layout.rightMargin: Kirigami.Units.largeSpacing
                    spacing: Kirigami.Units.smallSpacing

                    Rectangle {
                        width: Kirigami.Units.gridUnit * 0.6
                        height: width
                        radius: width / 2
                        color: Kirigami.Theme.negativeTextColor
                        SequentialAnimation on opacity {
                            running: DaemonClient.live
                            loops: Animation.Infinite
                            NumberAnimation { to: 0.3; duration: 600 }
                            NumberAnimation { to: 1; duration: 600 }
                        }
                    }
                    QQC2.Label {
                        text: i18np("Live: %1 contact", "Live: %1 contacts", DaemonClient.contacts.length)
                    }
                    QQC2.Label {
                        Layout.fillWidth: true
                        horizontalAlignment: Text.AlignRight
                        elide: Text.ElideRight
                        readonly property var g: DaemonClient.lastGesture
                        text: g.trigger ? i18nc("@info %1 gesture, %2 action", "Last gesture: %1 → %2", g.triggerTitle, g.actionLabel)
                                        : i18n("No gesture yet")
                        opacity: 0.8
                    }
                }

                // Numeric editors for the lines
                GridLayout {
                    id: editors
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.largeSpacing
                    Layout.rightMargin: Kirigami.Units.largeSpacing
                    Layout.bottomMargin: Kirigami.Units.largeSpacing
                    // Based on the column width, not our own (which depends on the column count).
                    columns: mainColumn.width > Kirigami.Units.gridUnit * 46 ? 2 : 1
                    columnSpacing: Kirigami.Units.gridUnit
                    rowSpacing: Kirigami.Units.largeSpacing

                    Repeater {
                        model: [
                            {section: "area", title: i18n("Gesture areas"), color: tpMap.areaColor},
                            {section: "side_drag", title: i18n("Side-drag bands"), color: tpMap.sideColor}
                        ]
                        delegate: ColumnLayout {
                            id: group
                            required property var modelData
                            spacing: Kirigami.Units.smallSpacing
                            Layout.fillWidth: true
                            Layout.alignment: Qt.AlignTop

                            RowLayout {
                                spacing: Kirigami.Units.smallSpacing
                                Rectangle {
                                    width: Kirigami.Units.gridUnit
                                    height: 3
                                    color: group.modelData.color
                                }
                                Kirigami.Heading {
                                    level: 4
                                    text: group.modelData.title
                                }
                            }
                            GridLayout {
                                columns: 4
                                columnSpacing: Kirigami.Units.smallSpacing
                                rowSpacing: Kirigami.Units.smallSpacing
                                Repeater {
                                    model: [
                                        {edge: "top", label: i18nc("@label edge", "Top")},
                                        {edge: "bottom", label: i18nc("@label edge", "Bottom")},
                                        {edge: "left", label: i18nc("@label edge", "Left")},
                                        {edge: "right", label: i18nc("@label edge", "Right")}
                                    ]
                                    delegate: RowLayout {
                                        required property var modelData
                                        Layout.columnSpan: 2
                                        spacing: Kirigami.Units.smallSpacing
                                        QQC2.Label {
                                            text: modelData.label
                                            Layout.minimumWidth: Kirigami.Units.gridUnit * 3
                                        }
                                        MmSpinBox {
                                            path: group.modelData.section + "." + modelData.edge
                                            maximum: tpMap.maxFor(group.modelData.section, modelData.edge)
                                            QQC2.ToolTip.visible: hovered
                                            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                                            QQC2.ToolTip.text: i18n("Distance from the edge; 0 disables this side. The centre keeps at least half of the touchpad.")
                                        }
                                    }
                                }
                            }
                        }
                    }

                    ColumnLayout {
                        spacing: Kirigami.Units.smallSpacing
                        Layout.fillWidth: true
                        Layout.alignment: Qt.AlignTop

                        RowLayout {
                            spacing: Kirigami.Units.smallSpacing
                            Rectangle {
                                width: Kirigami.Units.gridUnit * 0.7
                                height: width
                                color: Qt.rgba(tpMap.cornerColor.r, tpMap.cornerColor.g, tpMap.cornerColor.b, 0.35)
                                border.color: tpMap.cornerColor
                            }
                            Kirigami.Heading {
                                level: 4
                                text: i18n("Corners")
                            }
                        }
                        GridLayout {
                            columns: 4
                            columnSpacing: Kirigami.Units.smallSpacing
                            rowSpacing: Kirigami.Units.smallSpacing
                            QQC2.Label {
                                text: i18nc("@label corner size", "Width")
                                Layout.minimumWidth: Kirigami.Units.gridUnit * 3
                            }
                            MmSpinBox {
                                path: "corners.width"
                                minimum: 2
                                maximum: Math.min(30, tpMap.widthMm / 2)
                            }
                            QQC2.Label {
                                text: i18nc("@label corner size", "Height")
                                Layout.minimumWidth: Kirigami.Units.gridUnit * 3
                            }
                            MmSpinBox {
                                path: "corners.height"
                                minimum: 2
                                maximum: Math.min(30, tpMap.heightMm / 2)
                            }
                        }
                    }

                    ColumnLayout {
                        spacing: Kirigami.Units.smallSpacing
                        Layout.fillWidth: true
                        Layout.alignment: Qt.AlignTop
                        Kirigami.Heading {
                            level: 4
                            text: i18n("Touchpad")
                        }
                        QQC2.Label {
                            Layout.fillWidth: true
                            wrapMode: Text.Wrap
                            text: i18nc("@info %1 width, %2 height", "%1 × %2 mm, centre %3 × %4 mm",
                                        tpMap.widthMm.toLocaleString(Qt.locale(), "f", 1),
                                        tpMap.heightMm.toLocaleString(Qt.locale(), "f", 1),
                                        (tpMap.widthMm - tpMap.areaLeft - tpMap.areaRight).toLocaleString(Qt.locale(), "f", 1),
                                        (tpMap.heightMm - tpMap.areaTop - tpMap.areaBottom).toLocaleString(Qt.locale(), "f", 1))
                            opacity: 0.8
                        }
                    }
                }
            }
        }

        Kirigami.Separator {
            visible: page.sidePanel
            Layout.fillHeight: true
        }

        QQC2.ScrollView {
            id: sideScroll
            visible: page.sidePanel
            Layout.preferredWidth: Kirigami.Units.gridUnit * 21
            Layout.fillHeight: true
            contentWidth: availableWidth
            QQC2.ScrollBar.horizontal.policy: QQC2.ScrollBar.AlwaysOff

            Kirigami.Theme.colorSet: Kirigami.Theme.View
            Kirigami.Theme.inherit: false
            background: Rectangle {
                color: Kirigami.Theme.backgroundColor
            }

            ColumnLayout {
                width: sideScroll.availableWidth
                spacing: 0

                Kirigami.PlaceholderMessage {
                    visible: page.selectedZone === ""
                    Layout.fillWidth: true
                    Layout.topMargin: Kirigami.Units.gridUnit * 4
                    Layout.margins: Kirigami.Units.largeSpacing
                    icon.name: "input-touchpad"
                    text: i18n("Select a zone")
                    explanation: i18n("Click a corner, a side-drag band, a gesture area or the centre of the touchpad to edit its gestures.")
                }

                ZoneEditor {
                    id: sideEditor
                    visible: page.selectedZone !== ""
                    zone: page.selectedZone
                    previewTrigger: page.previewTrigger
                    onPreviewRequested: (trigger) => page.previewTrigger = trigger
                    onFingersChanged: {
                        const list = triggersFor(zone, fingers, DaemonClient.triggers);
                        if (zone === "centre" && !list.includes(page.previewTrigger))
                            page.previewTrigger = list.length > 0 ? list[0] : "";
                    }
                    Layout.fillWidth: true
                    Layout.margins: Kirigami.Units.largeSpacing
                }
            }
        }
    }

    Kirigami.Dialog {
        id: zoneDialog
        title: DaemonClient.zoneTitle(page.selectedZone)
        preferredWidth: Math.min(page.width - Kirigami.Units.gridUnit * 2, Kirigami.Units.gridUnit * 26)
        standardButtons: Kirigami.Dialog.Close
        padding: Kirigami.Units.largeSpacing

        ZoneEditor {
            showHeading: false
            zone: page.selectedZone
            previewTrigger: page.previewTrigger
            fingers: sideEditor.fingers
            onFingersChanged: sideEditor.fingers = fingers
            onPreviewRequested: (trigger) => page.previewTrigger = trigger
        }
    }
}
