// SPDX-License-Identifier: GPL-2.0-or-later
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import io.github.kaget

Kirigami.ApplicationWindow {
    id: root

    property bool screenshotMode: false
    property string currentPage: ""
    readonly property bool sidebarMode: width >= Kirigami.Units.gridUnit * 40

    title: DaemonClient.demo ? i18nc("@title:window", "KAGeT — demo") : i18nc("@title:window", "KAGeT")
    width: Kirigami.Units.gridUnit * 66
    height: Kirigami.Units.gridUnit * 44
    minimumWidth: Kirigami.Units.gridUnit * 20
    minimumHeight: Kirigami.Units.gridUnit * 22

    pageStack.globalToolBar.style: Kirigami.ApplicationHeaderStyle.ToolBar
    pageStack.globalToolBar.showNavigationButtons: Kirigami.ApplicationHeaderStyle.NoNavigationButtons
    pageStack.columnView.columnResizeMode: Kirigami.ColumnView.SingleColumn

    readonly property var pageComponents: ({
        "overview": overviewComponent,
        "map": mapComponent,
        "gestures": gesturesComponent,
        "settings": settingsComponent,
        "test": testComponent,
        "diagnostics": diagnosticsComponent,
    })

    function showPage(name: string) {
        if (currentPage === name && pageStack.depth > 0)
            return;
        currentPage = name;
        // Live touches are only shown on the map page.
        DaemonClient.setLive(false);
        pageStack.clear();
        pageStack.push(pageComponents[name]);
        if (globalDrawer.modal)
            globalDrawer.close();
    }

    function pickAction(trigger: string) {
        actionPicker.openFor(trigger, 0);
    }

    // Used by the hidden --screenshot mode to put the UI into a known state.
    function prepareScreenshot(state: string) {
        if (state === "reset") {
            actionPicker.close();
            setupDialog.close();
            DaemonClient.setLive(false);
            DaemonClient.revert();
            DaemonClient.clearError();
            if (globalDrawer.modal)
                globalDrawer.close();
            return;
        }
        const parts = state.split(":");
        const name = parts[0];
        const arg = parts.length > 1 ? parts.slice(1).join(":") : "";
        if (name === "setup" || name === "setup-missing") {
            showPage("overview");
            setupDialog.info = name === "setup"
                ? { unitInstalled: false, binary: "/home/user/.local/bin/kagetd", conflict: "asus-numberpad" }
                : { unitInstalled: false, binary: "", conflict: "" };
            setupDialog.open();
            return;
        }
        if (name === "drawer") {
            showPage("overview");
            globalDrawer.open();
            return;
        }
        if (name === "unsaved") {
            showPage("settings");
            DaemonClient.setDraftValue("corners.hold_ms", 400);
            return;
        }
        if (name === "apply-error") {
            // Bypass the UI clamp to show how SetConfig errors are reported.
            showPage("map");
            DaemonClient.setDraftValue("area.left", 30);
            DaemonClient.setDraftValue("area.right", 40);
            DaemonClient.apply();
            return;
        }
        if (name.startsWith("picker")) {
            showPage("gestures");
            actionPicker.openFor(arg, name === "picker-app" ? 4 : 0);
            return;
        }
        if (name.startsWith("gestures")) {
            if (name === "gestures-override")
                DaemonClient.setDraftValue("general.override_system_gestures", true);
            currentPage = "";
            showPage("gestures");
            if (arg !== "")
                pageStack.currentItem.setGroup(arg);
            return;
        }
        if (name.startsWith("map")) {
            currentPage = "";
            showPage("map");
            const page = pageStack.currentItem;
            if (name === "map-numberpad")
                page.numberPadOverlay = true;
            else if (name === "map-live")
                DaemonClient.setLive(true);
            else if (name === "map-hover")
                page.demoHover = true;
            else if (name === "map-side-on")
                DaemonClient.setDraftValue("side_drag.enabled", true);
            else if (name === "map-corners-off")
                DaemonClient.setDraftValue("corners.enabled", false);
            if (arg !== "")
                page.selectZone(arg);
            return;
        }
        currentPage = "";
        showPage(name);
    }

    globalDrawer: Kirigami.GlobalDrawer {
        id: drawer
        isMenu: false
        modal: !root.sidebarMode
        collapsible: false
        width: Kirigami.Units.gridUnit * 13
        handleVisible: modal
        onModalChanged: drawerOpen = !modal

        header: ColumnLayout {
            spacing: Kirigami.Units.smallSpacing
            Layout.fillWidth: true

            RowLayout {
                Layout.margins: Kirigami.Units.largeSpacing
                spacing: Kirigami.Units.largeSpacing

                Kirigami.Icon {
                    source: "input-touchpad"
                    implicitWidth: Kirigami.Units.iconSizes.large
                    implicitHeight: Kirigami.Units.iconSizes.large
                }
                ColumnLayout {
                    spacing: 0
                    Layout.fillWidth: true
                    Kirigami.Heading {
                        text: i18nc("@title application name", "KAGeT")
                        level: 2
                        Layout.fillWidth: true
                    }
                    QQC2.Label {
                        text: i18n("KDE Advanced Gesture Touchpad")
                        wrapMode: Text.Wrap
                        font: Kirigami.Theme.smallFont
                        opacity: 0.75
                        Layout.fillWidth: true
                    }
                }
            }
            Kirigami.Separator {
                Layout.fillWidth: true
            }
        }

        actions: [
            Kirigami.Action {
                text: i18nc("@action:inmenu page", "Overview")
                icon.name: "go-home"
                checkable: true
                checked: root.currentPage === "overview"
                onTriggered: root.showPage("overview")
            },
            Kirigami.Action {
                text: i18nc("@action:inmenu page", "Touchpad Map")
                icon.name: "input-touchpad"
                checkable: true
                checked: root.currentPage === "map"
                onTriggered: root.showPage("map")
            },
            Kirigami.Action {
                text: i18nc("@action:inmenu page", "Gestures")
                icon.name: "preferences-desktop-gestures-touch"
                checkable: true
                checked: root.currentPage === "gestures"
                onTriggered: root.showPage("gestures")
            },
            Kirigami.Action {
                text: i18nc("@action:inmenu page", "Settings")
                icon.name: "configure"
                checkable: true
                checked: root.currentPage === "settings"
                onTriggered: root.showPage("settings")
            },
            Kirigami.Action {
                text: i18nc("@action:inmenu page", "Test Touchpad")
                icon.name: "debug-run"
                checkable: true
                checked: root.currentPage === "test"
                onTriggered: root.showPage("test")
            },
            Kirigami.Action {
                text: i18nc("@action:inmenu page", "Diagnostics")
                icon.name: "tools-report-bug"
                checkable: true
                checked: root.currentPage === "diagnostics"
                onTriggered: root.showPage("diagnostics")
            }
        ]
    }

    Component {
        id: overviewComponent
        OverviewPage {}
    }
    Component {
        id: mapComponent
        TouchpadMapPage {}
    }
    Component {
        id: gesturesComponent
        GesturesPage {}
    }
    Component {
        id: settingsComponent
        SettingsPage {}
    }
    Component {
        id: testComponent
        TestPage {}
    }
    Component {
        id: diagnosticsComponent
        DiagnosticsPage {}
    }

    ActionPickerDialog {
        id: actionPicker
    }

    Connections {
        target: DaemonClient
        function onNotify(text: string, isError: bool) {
            root.showPassiveNotification(text, isError ? "long" : "short");
        }
        function onSetupNeeded(info) {
            setupDialog.info = info;
            setupDialog.open();
        }
    }

    Kirigami.PromptDialog {
        id: setupDialog
        property var info: ({})
        readonly property bool missing: !(info.unitInstalled ?? false) && (info.binary ?? "") === ""
        title: missing ? i18nc("@title:dialog", "KAGeT daemon not found") : i18nc("@title:dialog", "Start the KAGeT daemon")
        subtitle: {
            if (missing)
                return i18n("kagetd is not installed and no kagetd program was found. Build it with “cargo build --release” in the KAGeT folder or install the KAGeT package, then try again.");
            let lines = [];
            if (!(info.unitInstalled ?? false))
                lines.push(i18n("Install kagetd as a user service (%1). It will start automatically when you log in.", info.binary));
            if ((info.conflict ?? "") !== "")
                lines.push(i18n("Stop and disable “%1”. It holds the touchpad exclusively, so KAGeT cannot read it while it runs.", info.conflict));
            lines.push(i18n("Start kagetd."));
            return lines.join("\n\n");
        }
        standardButtons: missing ? Kirigami.Dialog.Close : Kirigami.Dialog.NoButton
        customFooterActions: [
            Kirigami.Action {
                text: i18nc("@action:button", "Set Up and Start")
                icon.name: "media-playback-start"
                visible: !setupDialog.missing
                onTriggered: {
                    setupDialog.close();
                    DaemonClient.installAndStart(true);
                }
            },
            Kirigami.Action {
                text: i18nc("@action:button", "Cancel")
                icon.name: "dialog-cancel"
                visible: !setupDialog.missing
                onTriggered: setupDialog.close()
            }
        ]
    }

    Component.onCompleted: {
        drawer.drawerOpen = !drawer.modal;
        showPage("overview");
    }
}
