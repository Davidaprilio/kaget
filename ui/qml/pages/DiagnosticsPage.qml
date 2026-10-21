// SPDX-License-Identifier: GPL-2.0-or-later
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard
import io.github.kaget

FormCard.FormCardPage {
    id: page

    title: i18nc("@title", "Diagnostics")
    footer: ApplyBar {}

    readonly property var st: DaemonClient.state
    readonly property var device: st.device ?? ({})
    readonly property var numberpad: st.numberpad ?? ({})
    readonly property bool hasDevice: device.name !== undefined
    readonly property string logCommand: "journalctl --user -u kagetd -f"
    readonly property real cardWidth: Kirigami.Units.gridUnit * 36

    function hex(v) {
        if (v === undefined || v === null)
            return "?";
        if (typeof v === "string")
            return v;
        return ("0000" + Number(v).toString(16).toUpperCase()).slice(-4);
    }
    function yesNo(b) {
        return b ? i18n("yes") : i18n("no");
    }
    function report() {
        const lines = [];
        lines.push("KAGeT diagnostics");
        lines.push("Daemon: " + (DaemonClient.demo ? "demo" : DaemonClient.connected ? "running (" + DaemonClient.serviceOwner + ")" : "not running"));
        lines.push("Multitouch: " + DaemonClient.multitouchLevel());
        for (const c of Helpers.accessChecks)
            lines.push((c.ok ? "[ok] " : "[!!] ") + c.title + ": " + c.detail.replace(/\n/g, "; "));
        lines.push("State: " + DaemonClient.stateJson);
        return lines.join("\n");
    }

    Component.onCompleted: Helpers.runAccessChecks()

    // ----- daemon -----
    FormCard.FormHeader {
        title: i18nc("@title:group", "Daemon")
        maximumWidth: page.cardWidth
    }
    FormCard.FormCard {
        maximumWidth: page.cardWidth

        InfoRow {
            label: i18n("Status")
            iconName: DaemonClient.connected ? "emblem-ok-symbolic" : "emblem-error"
            value: DaemonClient.demo ? i18n("Demo mode (simulated daemon)")
                 : DaemonClient.connected ? i18n("Running") : DaemonClient.probing ? i18n("Checking…") : i18n("Not running")
        }
        FormCard.FormDelegateSeparator {}
        InfoRow {
            label: i18n("Input mode")
            value: DaemonClient.mode === "passive" ? i18n("Passive (reads the touchpad alongside KDE)")
                 : DaemonClient.mode === "filter" ? i18n("Filter (exclusive grab and virtual touchpad)")
                 : (DaemonClient.mode !== "" ? DaemonClient.mode : "—")
        }
        FormCard.FormDelegateSeparator {}
        InfoRow {
            label: i18n("Version")
            value: page.st.version !== undefined ? String(page.st.version) : "—"
        }
        FormCard.FormDelegateSeparator {}
        InfoRow {
            label: i18n("D-Bus service")
            value: DaemonClient.serviceName
            monospace: true
        }
        FormCard.FormDelegateSeparator {}
        InfoRow {
            label: i18n("Object / interface")
            value: DaemonClient.objectPath + "\n" + DaemonClient.interfaceName
            monospace: true
        }
        FormCard.FormDelegateSeparator {}
        InfoRow {
            label: i18n("Bus owner")
            value: DaemonClient.serviceOwner !== "" ? DaemonClient.serviceOwner : "—"
            monospace: true
        }
        FormCard.FormDelegateSeparator {}
        FormCard.FormButtonDelegate {
            icon.name: "document-edit"
            text: i18n("Configuration file")
            description: page.st.config_path ?? "~/.config/kaget/config.toml"
            QQC2.ToolTip.text: i18n("Open in the default text editor")
            QQC2.ToolTip.visible: hovered
            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
            onClicked: DaemonClient.openConfigFile(page.st.config_path ?? "~/.config/kaget/config.toml")
        }
        FormCard.FormDelegateSeparator {}
        FormCard.FormButtonDelegate {
            icon.name: DaemonClient.connected ? "view-refresh" : "media-playback-start"
            text: DaemonClient.connected ? i18n("Restart daemon") : i18n("Start daemon")
            description: DaemonClient.connected ? "systemctl --user restart kagetd" : "systemctl --user start kagetd"
            enabled: !DaemonClient.startingDaemon
            onClicked: DaemonClient.connected ? DaemonClient.restartDaemon() : DaemonClient.startDaemon()
        }
        FormCard.FormDelegateSeparator {}
        FormCard.FormButtonDelegate {
            icon.name: "view-refresh"
            text: i18n("Reload state from the daemon")
            enabled: DaemonClient.connected
            onClicked: DaemonClient.refresh()
        }
    }

    // ----- log -----
    FormCard.FormHeader {
        title: i18nc("@title:group", "Daemon log")
        maximumWidth: page.cardWidth
    }
    FormCard.FormCard {
        maximumWidth: page.cardWidth
        FormCard.AbstractFormDelegate {
            background: null
            hoverEnabled: false
            contentItem: ColumnLayout {
                spacing: Kirigami.Units.smallSpacing
                QQC2.Label {
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    text: i18n("The daemon logs to the systemd journal. Run this in a terminal to follow it:")
                }
                RowLayout {
                    Layout.fillWidth: true
                    QQC2.TextField {
                        Layout.fillWidth: true
                        readOnly: true
                        text: page.logCommand
                        font.family: "monospace"
                    }
                    QQC2.Button {
                        icon.name: "edit-copy"
                        text: i18nc("@action:button", "Copy")
                        onClicked: {
                            Helpers.copyToClipboard(page.logCommand);
                            applicationWindow().showPassiveNotification(i18n("Copied to the clipboard"));
                        }
                    }
                }
            }
        }
    }

    // ----- device -----
    FormCard.FormHeader {
        title: i18nc("@title:group", "Device capabilities")
        maximumWidth: page.cardWidth
    }
    FormCard.FormCard {
        maximumWidth: page.cardWidth

        InfoRow {
            label: i18n("Name")
            value: page.hasDevice ? page.device.name : i18n("No touchpad detected")
        }
        FormCard.FormDelegateSeparator { visible: page.hasDevice }
        InfoRow {
            visible: page.hasDevice
            label: i18n("Vendor:product")
            value: page.hex(page.device.vendor) + ":" + page.hex(page.device.product)
            monospace: true
        }
        FormCard.FormDelegateSeparator { visible: page.hasDevice }
        InfoRow {
            visible: page.hasDevice
            label: i18n("Size")
            value: i18nc("@info %1 width, %2 height", "%1 × %2 mm",
                         Number(page.device.width_mm ?? 0).toLocaleString(Qt.locale(), "f", 1),
                         Number(page.device.height_mm ?? 0).toLocaleString(Qt.locale(), "f", 1))
        }
        FormCard.FormDelegateSeparator { visible: page.hasDevice }
        InfoRow {
            visible: page.hasDevice
            label: i18n("Resolution")
            value: (page.device.resolution_x ?? 0) > 0
                ? i18nc("@info %1 x resolution, %2 y resolution", "%1 × %2 units/mm", page.device.resolution_x, page.device.resolution_y)
                : i18n("Not reported (using the fallback size from the configuration)")
        }
        FormCard.FormDelegateSeparator { visible: page.hasDevice }
        InfoRow {
            visible: page.hasDevice
            label: i18n("Contacts (MT slots)")
            value: String(page.device.max_fingers ?? "?")
        }
        FormCard.FormDelegateSeparator { visible: page.hasDevice }
        InfoRow {
            visible: page.hasDevice
            label: i18n("Semi-multitouch")
            value: page.yesNo(page.device.semi_mt ?? false)
        }
        FormCard.FormDelegateSeparator { visible: page.hasDevice }
        InfoRow {
            visible: page.hasDevice
            label: i18n("Multitouch level")
            value: (DaemonClient.state, DaemonClient.multitouchLevel())
        }
        FormCard.FormDelegateSeparator { visible: page.hasDevice }
        InfoRow {
            visible: page.hasDevice
            label: i18n("Multi-finger gestures")
            readonly property bool ok: (page.device.max_fingers ?? 0) >= 3 && !(page.device.semi_mt ?? false)
            value: ok ? i18n("Supported") : i18n("Not supported (needs 3 or more fingers and full multitouch)")
            valueColor: ok ? Kirigami.Theme.positiveTextColor : Kirigami.Theme.negativeTextColor
        }
        FormCard.FormDelegateSeparator {}
        InfoRow {
            label: i18n("NumberPad")
            value: (page.numberpad.supported ?? false)
                ? i18np("Supported, %1 key", "Supported, %1 keys", (page.numberpad.keys ?? []).length) + (page.numberpad.active ? i18n(", active") : "")
                : i18n("Not supported")
        }
    }

    // ----- access checks -----
    FormCard.FormHeader {
        title: i18nc("@title:group", "Access checks (this computer)")
        maximumWidth: page.cardWidth
    }
    FormCard.FormCard {
        maximumWidth: page.cardWidth

        Repeater {
            model: Helpers.accessChecks
            delegate: InfoRow {
                required property var modelData
                required property int index
                label: modelData.title
                value: modelData.detail
                iconName: modelData.ok ? "emblem-ok-symbolic" : (modelData.informational ? "dialog-information" : "emblem-warning")
            }
        }
        FormCard.FormButtonDelegate {
            icon.name: "view-refresh"
            text: Helpers.checkingAccess ? i18n("Checking…") : i18n("Run checks again")
            enabled: !Helpers.checkingAccess
            onClicked: Helpers.runAccessChecks()
        }
    }

    // ----- raw state -----
    FormCard.FormHeader {
        title: i18nc("@title:group", "Raw state")
        maximumWidth: page.cardWidth
    }
    FormCard.FormCard {
        maximumWidth: page.cardWidth
        Layout.bottomMargin: Kirigami.Units.gridUnit

        FormCard.FormButtonDelegate {
            id: rawToggle
            property bool expanded: false
            icon.name: expanded ? "go-up" : "go-down"
            text: expanded ? i18n("Hide GetState() JSON") : i18n("Show GetState() JSON")
            onClicked: expanded = !expanded
        }
        FormCard.AbstractFormDelegate {
            visible: rawToggle.expanded
            background: null
            hoverEnabled: false
            contentItem: QQC2.TextArea {
                readOnly: true
                text: DaemonClient.stateJson
                font.family: "monospace"
                wrapMode: TextEdit.WrapAnywhere
            }
        }
        FormCard.FormDelegateSeparator {}
        FormCard.FormButtonDelegate {
            icon.name: "edit-copy"
            text: i18n("Copy diagnostics report")
            onClicked: {
                Helpers.copyToClipboard(page.report());
                applicationWindow().showPassiveNotification(i18n("Copied to the clipboard"));
            }
        }
    }
}
