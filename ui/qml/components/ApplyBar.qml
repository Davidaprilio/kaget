// SPDX-License-Identifier: GPL-2.0-or-later
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import io.github.kaget

// Window footer: daemon-not-running banner, SetConfig errors and the
// Apply / Revert bar for the edited configuration copy.
ColumnLayout {
    id: bar
    spacing: 0
    // Children report visible=false while the parent is hidden, so use the conditions.
    readonly property bool showNotRunning: !DaemonClient.demo && !DaemonClient.connected && !DaemonClient.probing
    property bool showError: false
    Component.onCompleted: showError = DaemonClient.lastError !== ""
    visible: showNotRunning || showError || DaemonClient.externalChange || DaemonClient.dirty

    Kirigami.InlineMessage {
        id: notRunning
        Layout.fillWidth: true
        position: Kirigami.InlineMessage.Position.Footer
        type: Kirigami.MessageType.Warning
        visible: bar.showNotRunning
        text: i18n("The KAGeT daemon is not running. Gestures are inactive and changes cannot be applied.")
        actions: [
            Kirigami.Action {
                text: DaemonClient.startingDaemon ? i18n("Starting…") : i18n("Start daemon")
                icon.name: "media-playback-start"
                enabled: !DaemonClient.startingDaemon
                onTriggered: DaemonClient.startDaemon()
            }
        ]
    }

    Kirigami.InlineMessage {
        id: errorMessage
        Layout.fillWidth: true
        position: Kirigami.InlineMessage.Position.Footer
        type: Kirigami.MessageType.Error
        showCloseButton: true
        visible: bar.showError
        text: DaemonClient.lastError
        onVisibleChanged: {
            // The close button hides the message.
            if (!visible && bar.showError && bar.visible) {
                bar.showError = false;
                DaemonClient.clearError();
            }
        }
        Connections {
            target: DaemonClient
            function onLastErrorChanged() {
                bar.showError = DaemonClient.lastError !== "";
                errorMessage.visible = bar.showError;
            }
        }
    }

    Kirigami.InlineMessage {
        id: external
        Layout.fillWidth: true
        position: Kirigami.InlineMessage.Position.Footer
        type: Kirigami.MessageType.Information
        visible: DaemonClient.externalChange
        text: i18n("The configuration file was changed outside of this window. Revert to load it, or Apply to overwrite it.")
    }

    QQC2.ToolBar {
        id: applyBar
        Layout.fillWidth: true
        position: QQC2.ToolBar.Footer
        visible: DaemonClient.dirty

        contentItem: RowLayout {
            spacing: Kirigami.Units.smallSpacing

            Kirigami.Icon {
                source: "document-edit"
                implicitWidth: Kirigami.Units.iconSizes.smallMedium
                implicitHeight: Kirigami.Units.iconSizes.smallMedium
                Layout.leftMargin: Kirigami.Units.smallSpacing
            }
            QQC2.Label {
                text: DaemonClient.demo ? i18n("Unsaved changes (demo: kept in memory only)") : i18n("You have unsaved changes")
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            QQC2.Button {
                text: i18nc("@action:button", "Revert")
                icon.name: "edit-undo"
                enabled: !DaemonClient.applying
                onClicked: DaemonClient.revert()
            }
            QQC2.Button {
                text: DaemonClient.applying ? i18nc("@action:button", "Applying…") : i18nc("@action:button", "Apply")
                icon.name: "dialog-ok-apply"
                highlighted: true
                enabled: DaemonClient.connected && !DaemonClient.applying
                onClicked: DaemonClient.apply()
            }
        }
    }
}
