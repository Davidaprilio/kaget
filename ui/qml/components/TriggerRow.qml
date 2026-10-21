// SPDX-License-Identifier: GPL-2.0-or-later
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import io.github.kaget

// One trigger with its action picker, `when_running` selector (app actions),
// Test button and Remove button. Used by the Gestures page and the zone editor.
QQC2.ItemDelegate {
    id: row

    required property var info
    property bool compact: false
    property bool selected: false
    property bool selectable: false

    signal previewRequested()

    readonly property var cfg: DaemonClient.draft
    readonly property bool overrideSystem: cfg.general?.override_system_gestures ?? false
    readonly property var bindingObj: {
        const list = cfg.bindings ?? [];
        for (let i = 0; i < list.length; ++i) {
            if (list[i].trigger === info.trigger)
                return list[i];
        }
        return null;
    }
    readonly property string boundAction: bindingObj?.action ?? ""
    readonly property string whenRunning: bindingObj?.when_running ?? "new"
    readonly property bool systemOwned: info.systemOwned ?? false
    readonly property bool passiveLocked: systemOwned && DaemonClient.passive
    readonly property bool available: (info.available ?? true) && !passiveLocked
    readonly property bool lockedBySystem: systemOwned && (!overrideSystem || DaemonClient.passive)
    readonly property bool editable: available && !lockedBySystem
    readonly property string effectiveAction: (systemOwned && boundAction === "") ? "system" : boundAction
    readonly property var actionData: (DaemonClient.actions, DaemonClient.actionInfo(effectiveAction))
    readonly property bool sliderCapable: info.kind === "corner_swipe" || info.kind === "swipe" || info.kind === "sidedrag"
    readonly property bool testable: DaemonClient.connected && effectiveAction !== "" && effectiveAction !== "system" && effectiveAction !== "none"

    hoverEnabled: selectable
    highlighted: selected
    down: false
    onClicked: if (selectable) row.previewRequested()

    topPadding: Kirigami.Units.smallSpacing * 1.5
    bottomPadding: Kirigami.Units.smallSpacing * 1.5
    leftPadding: Kirigami.Units.largeSpacing
    rightPadding: Kirigami.Units.largeSpacing

    contentItem: GridLayout {
        columns: row.compact ? 1 : 2
        columnSpacing: Kirigami.Units.largeSpacing
        rowSpacing: Kirigami.Units.smallSpacing

        ColumnLayout {
            spacing: 0
            Layout.fillWidth: true
            Layout.minimumWidth: Kirigami.Units.gridUnit * 8
            opacity: row.editable ? 1 : 0.6

            Flow {
                id: titleFlow
                spacing: Kirigami.Units.smallSpacing
                Layout.fillWidth: true

                QQC2.Label {
                    text: row.info.title
                    font.weight: Font.DemiBold
                    elide: Text.ElideRight
                    width: Math.min(implicitWidth, titleFlow.width)
                }
                Rectangle {
                    // "System" tag for system-owned triggers.
                    visible: row.systemOwned
                    implicitWidth: sysLabel.implicitWidth + Kirigami.Units.smallSpacing * 2
                    implicitHeight: sysLabel.implicitHeight + 2
                    radius: height / 2
                    color: "transparent"
                    border.width: 1
                    border.color: Kirigami.Theme.disabledTextColor
                    QQC2.Label {
                        id: sysLabel
                        anchors.centerIn: parent
                        text: i18nc("@label tag", "System")
                        font: Kirigami.Theme.smallFont
                        color: Kirigami.Theme.disabledTextColor
                    }
                }
                Rectangle {
                    // "Slider" tag when the bound action has an inverse.
                    visible: row.sliderCapable && (row.actionData.slider ?? false)
                    implicitWidth: sliderLabel.implicitWidth + Kirigami.Units.smallSpacing * 2
                    implicitHeight: sliderLabel.implicitHeight + 2
                    radius: height / 2
                    color: Kirigami.ColorUtils.tintWithAlpha(Kirigami.Theme.backgroundColor, Kirigami.Theme.highlightColor, 0.18)
                    QQC2.Label {
                        id: sliderLabel
                        anchors.centerIn: parent
                        text: i18nc("@label tag", "Slider")
                        font: Kirigami.Theme.smallFont
                    }
                    QQC2.ToolTip.visible: sliderHover.hovered
                    QQC2.ToolTip.text: i18n("This action has an inverse: the gesture works like a slider, one step per slider step, moving back reverses.")
                    HoverHandler {
                        id: sliderHover
                    }
                }
            }

            QQC2.Label {
                Layout.fillWidth: true
                visible: text !== ""
                text: row.passiveLocked ? (row.info.reason || i18n("Used by KDE"))
                    : !row.available ? (row.info.reason || i18n("Not available on this touchpad."))
                    : row.lockedBySystem ? i18n("Handled by the desktop. Enable “Override system gestures” to rebind it.")
                    : (row.info.description ?? "")
                color: !row.available ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.textColor
                opacity: !row.available ? 1 : 0.7
                font: Kirigami.Theme.smallFont
                wrapMode: Text.Wrap
                maximumLineCount: 2
                elide: Text.ElideRight
            }
        }

        RowLayout {
            spacing: Kirigami.Units.smallSpacing
            Layout.fillWidth: row.compact
            Layout.alignment: Qt.AlignRight | Qt.AlignVCenter

            QQC2.Button {
                id: actionButton
                enabled: row.editable
                Layout.fillWidth: row.compact
                Layout.preferredWidth: row.compact ? -1 : Kirigami.Units.gridUnit * 13
                Layout.minimumWidth: Kirigami.Units.gridUnit * 6
                onClicked: applicationWindow().pickAction(row.info.trigger)
                QQC2.ToolTip.visible: hovered && row.actionData.action !== ""
                QQC2.ToolTip.text: row.actionData.action
                QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                Accessible.name: i18nc("@action:button %1 current action", "Choose action, current: %1", row.actionData.label)

                contentItem: RowLayout {
                    spacing: Kirigami.Units.smallSpacing
                    Kirigami.Icon {
                        source: row.actionData.icon || "list-add"
                        implicitWidth: Kirigami.Units.iconSizes.small
                        implicitHeight: Kirigami.Units.iconSizes.small
                        opacity: row.effectiveAction === "" ? 0.6 : 1
                    }
                    QQC2.Label {
                        text: row.actionData.label
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                        opacity: row.effectiveAction === "" ? 0.6 : 1
                        font.italic: row.effectiveAction === "" || row.effectiveAction === "system"
                    }
                    Kirigami.Icon {
                        source: "arrow-down"
                        implicitWidth: Kirigami.Units.iconSizes.small
                        implicitHeight: Kirigami.Units.iconSizes.small
                        opacity: 0.6
                    }
                }
            }

            QQC2.ComboBox {
                id: whenCombo
                visible: row.boundAction.startsWith("app:")
                enabled: row.editable
                Layout.preferredWidth: Kirigami.Units.gridUnit * 8
                textRole: "text"
                valueRole: "value"
                model: [
                    {value: "new", text: i18nc("@item:inlistbox when_running", "New instance")},
                    {value: "close", text: i18nc("@item:inlistbox when_running", "Close")},
                    {value: "focus", text: i18nc("@item:inlistbox when_running", "Focus")},
                    {value: "minimize", text: i18nc("@item:inlistbox when_running", "Minimize")},
                    {value: "toggle", text: i18nc("@item:inlistbox when_running", "Toggle")}
                ]
                currentIndex: Math.max(0, ["new", "close", "focus", "minimize", "toggle"].indexOf(row.whenRunning))
                onActivated: DaemonClient.setBinding(row.info.trigger, row.boundAction, currentValue)
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: i18n("What to do when the application is already running")
                QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
            }

            QQC2.ToolButton {
                icon.name: "media-playback-start"
                enabled: row.editable && row.testable
                display: QQC2.AbstractButton.IconOnly
                text: i18nc("@action:button", "Test")
                onClicked: DaemonClient.runAction(row.effectiveAction)
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: i18n("Test this action now")
                QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
            }

            QQC2.ToolButton {
                icon.name: "edit-clear"
                enabled: row.editable && row.boundAction !== ""
                opacity: row.boundAction !== "" ? 1 : 0.35
                display: QQC2.AbstractButton.IconOnly
                text: i18nc("@action:button", "Remove binding")
                onClicked: DaemonClient.setBinding(row.info.trigger, "")
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: row.systemOwned ? i18n("Back to the system default") : i18n("Remove this binding")
                QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
            }
        }
    }
}
