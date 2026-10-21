// SPDX-License-Identifier: GPL-2.0-or-later
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard
import io.github.kaget

FormCard.FormCardPage {
    id: page

    title: i18nc("@title", "Overview")
    footer: ApplyBar {}

    readonly property bool running: DaemonClient.connected
    readonly property var st: DaemonClient.state
    readonly property var device: st.device ?? ({})
    readonly property var numberpad: st.numberpad ?? ({})
    readonly property bool hasDevice: device.name !== undefined
    readonly property real cardWidth: Kirigami.Units.gridUnit * 36

    function mm(v) {
        return Number(v ?? 0).toLocaleString(Qt.locale(), "f", 1);
    }

    // ----- banner -----
    ColumnLayout {
        Layout.fillWidth: true
        Layout.maximumWidth: page.cardWidth
        Layout.alignment: Qt.AlignHCenter
        Layout.topMargin: Kirigami.Units.gridUnit
        Layout.leftMargin: Kirigami.Units.largeSpacing
        Layout.rightMargin: Kirigami.Units.largeSpacing
        spacing: Kirigami.Units.smallSpacing

        RowLayout {
            spacing: Kirigami.Units.largeSpacing
            Kirigami.Icon {
                source: "input-touchpad"
                implicitWidth: Kirigami.Units.iconSizes.huge
                implicitHeight: Kirigami.Units.iconSizes.huge
            }
            ColumnLayout {
                spacing: 0
                Layout.fillWidth: true
                Kirigami.Heading {
                    text: i18nc("@title application name", "KAGeT")
                    level: 1
                }
                QQC2.Label {
                    text: i18n("KDE Advanced Gesture Touchpad")
                    opacity: 0.8
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                }
            }
        }

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
            visible: DaemonClient.demo
            type: Kirigami.MessageType.Information
            text: i18n("Demo mode: showing bundled sample data. No daemon is contacted and nothing is saved.")
        }
    }

    // ----- not running -----
    FormCard.FormCard {
        visible: !page.running && !DaemonClient.probing
        maximumWidth: page.cardWidth
        Layout.topMargin: Kirigami.Units.largeSpacing

        FormCard.AbstractFormDelegate {
            background: null
            hoverEnabled: false
            contentItem: Kirigami.PlaceholderMessage {
                icon.name: "dialog-warning"
                text: i18n("The KAGeT daemon is not running")
                explanation: i18n("kagetd grabs the touchpad and recognises the gestures. Start it as a systemd user service:\nsystemctl --user start kagetd")
                helpfulAction: Kirigami.Action {
                    text: DaemonClient.startingDaemon ? i18n("Starting…") : i18n("Start daemon")
                    icon.name: "media-playback-start"
                    enabled: !DaemonClient.startingDaemon
                    onTriggered: DaemonClient.startDaemon()
                }
            }
        }
    }

    // ----- status -----
    FormCard.FormHeader {
        title: i18nc("@title:group", "Status")
        maximumWidth: page.cardWidth
    }
    FormCard.FormCard {
        maximumWidth: page.cardWidth

        InfoRow {
            label: i18n("Daemon")
            iconName: page.running ? "emblem-ok-symbolic" : "emblem-error"
            value: DaemonClient.demo ? i18n("Simulated (demo)")
                 : page.running ? (page.st.version !== undefined ? i18n("Running, version %1", String(page.st.version)) : i18n("Running"))
                 : DaemonClient.probing ? i18n("Checking…") : i18n("Not running")
        }
        FormCard.FormDelegateSeparator { visible: DaemonClient.mode !== "" }
        InfoRow {
            visible: DaemonClient.mode !== ""
            label: i18n("Input mode")
            value: DaemonClient.passive ? i18n("Passive: KDE keeps its own gestures") : i18n("Filter")
        }
        FormCard.FormDelegateSeparator {}
        FormCard.FormSwitchDelegate {
            text: i18n("Gestures enabled")
            description: i18n("Turns KAGeT on or off right away, without changing the saved configuration.")
            enabled: page.running
            checked: page.st.enabled ?? false
            onToggled: DaemonClient.setDaemonEnabled(checked)
        }
        FormCard.FormDelegateSeparator {}
        FormCard.FormButtonDelegate {
            icon.name: (page.st.paused ?? false) ? "media-playback-start" : "media-playback-pause"
            text: (page.st.paused ?? false) ? i18n("Resume gestures") : i18n("Pause gestures")
            description: (page.st.paused ?? false) ? i18n("KAGeT is paused: the touchpad behaves like a plain touchpad.")
                                                   : i18n("Temporarily pass every touch through to the desktop.")
            enabled: page.running
            onClicked: DaemonClient.togglePause()
        }
    }

    // ----- touchpad -----
    FormCard.FormHeader {
        title: i18nc("@title:group", "Touchpad")
        maximumWidth: page.cardWidth
    }
    FormCard.FormCard {
        maximumWidth: page.cardWidth

        FormCard.AbstractFormDelegate {
            visible: page.hasDevice
            background: null
            onClicked: applicationWindow().showPage("map")
            contentItem: TouchpadMap {
                interactive: false
                showLabels: false
                implicitHeight: width / aspect
                QQC2.ToolTip.visible: parent.hovered
                QQC2.ToolTip.text: i18n("Open the Touchpad Map")
            }
        }
        InfoRow {
            label: i18n("Device")
            value: page.hasDevice ? page.device.name : i18n("No touchpad detected")
        }
        FormCard.FormDelegateSeparator {}
        InfoRow {
            visible: page.hasDevice
            label: i18n("Size")
            value: i18nc("@info %1 width, %2 height", "%1 × %2 mm", page.mm(page.device.width_mm), page.mm(page.device.height_mm))
        }
        FormCard.FormDelegateSeparator { visible: page.hasDevice }
        InfoRow {
            visible: page.hasDevice
            label: i18n("Multitouch")
            value: (DaemonClient.state, DaemonClient.multitouchLevel())
            valueColor: (page.device.max_fingers ?? 0) >= 3 && !(page.device.semi_mt ?? false) ? Kirigami.Theme.textColor : Kirigami.Theme.neutralTextColor
        }
        FormCard.FormDelegateSeparator { visible: page.hasDevice }
        InfoRow {
            visible: page.hasDevice
            label: i18n("Max fingers")
            value: String(page.device.max_fingers ?? "?")
        }
    }

    // ----- NumberPad -----
    FormCard.FormHeader {
        title: i18nc("@title:group", "NumberPad")
        maximumWidth: page.cardWidth
    }
    FormCard.FormCard {
        maximumWidth: page.cardWidth

        InfoRow {
            label: i18n("NumberPad")
            iconName: "accessories-calculator"
            value: !(page.numberpad.supported ?? false) ? i18n("Not supported on this touchpad")
                 : (page.numberpad.active ?? false) ? i18n("Active") : i18n("Supported, currently off")
        }
        FormCard.FormDelegateSeparator { visible: page.numberpad.supported ?? false }
        InfoRow {
            visible: page.numberpad.supported ?? false
            label: i18n("Backlight level")
            value: String(page.numberpad.level ?? 0)
        }
        FormCard.FormDelegateSeparator { visible: page.numberpad.supported ?? false }
        FormCard.FormButtonDelegate {
            visible: page.numberpad.supported ?? false
            icon.name: "accessories-calculator"
            text: (page.numberpad.active ?? false) ? i18n("Turn NumberPad off") : i18n("Turn NumberPad on")
            enabled: page.running
            onClicked: DaemonClient.runAction("numberpad.toggle")
        }
    }

    // ----- shortcuts to other pages -----
    FormCard.FormHeader {
        title: i18nc("@title:group", "Configure")
        maximumWidth: page.cardWidth
    }
    FormCard.FormCard {
        maximumWidth: page.cardWidth
        Layout.bottomMargin: Kirigami.Units.gridUnit

        FormCard.FormButtonDelegate {
            icon.name: "input-touchpad"
            text: i18n("Touchpad Map")
            description: i18n("Resize gesture areas, corners and side-drag bands")
            onClicked: applicationWindow().showPage("map")
        }
        FormCard.FormDelegateSeparator {}
        FormCard.FormButtonDelegate {
            icon.name: "preferences-desktop-gestures-touch"
            text: i18n("Gestures")
            description: (DaemonClient.draft, i18np("%1 gesture bound", "%1 gestures bound", DaemonClient.boundCount()))
            onClicked: applicationWindow().showPage("gestures")
        }
    }
}
