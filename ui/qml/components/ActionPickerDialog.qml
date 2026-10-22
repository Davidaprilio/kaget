// SPDX-License-Identifier: GPL-2.0-or-later
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import io.github.kaget

// Action picker: searchable list from ListActions() plus editors for
// key:..., shortcut:..., cmd:... and app:<desktop id> (with when_running).
Kirigami.Dialog {
    id: dialog

    property string trigger: ""
    property var tinfo: ({})
    property string current: ""
    property string pending: ""
    property string pendingWhenRunning: "new"
    property string appId: ""
    /// Choosing a control panel button instead of a gesture binding: the slot to replace, -1 to
    /// add one, -2 when picking for a gesture.
    property int panelIndex: -2
    readonly property bool panelMode: panelIndex > -2
    property var apps: []
    readonly property bool systemOwned: tinfo.systemOwned ?? false
    readonly property var window: QQC2.ApplicationWindow.window

    readonly property string chosenAction: {
        switch (tabs.currentIndex) {
        case 0: return pending;
        case 1: return keyField.text.trim() !== "" ? "key:" + keyField.text.trim() : "";
        case 2: return shortcutField.text.trim() !== "" ? "shortcut:" + shortcutField.text.trim() : "";
        case 3: return cmdField.text.trim() !== "" ? "cmd:" + cmdField.text.trim() : "";
        case 4: return appId !== "" ? "app:" + appId : "";
        }
        return "";
    }
    readonly property bool valid: {
        switch (tabs.currentIndex) {
        case 0: return !panelMode || (pending !== "" && pending !== "none");
        case 1: return Helpers.isValidKeyCombo(keyField.text.trim());
        case 2: return shortcutField.text.trim() !== "";
        case 3: return cmdField.text.trim() !== "";
        case 4: return appId !== "";
        }
        return false;
    }

    title: panelMode ? i18n("Control panel button") : (tinfo.longTitle ?? i18n("Choose action"))
    preferredWidth: Math.min((window?.width ?? 800) - Kirigami.Units.gridUnit * 2, Kirigami.Units.gridUnit * 34)
    padding: 0
    standardButtons: Kirigami.Dialog.Cancel
    customFooterActions: [
        Kirigami.Action {
            text: i18nc("@action:button", "Use this action")
            icon.name: "dialog-ok"
            enabled: dialog.valid
            onTriggered: dialog.commit()
        }
    ]

    function kindTab(action: string): int {
        if (action.startsWith("key:")) return 1;
        if (action.startsWith("shortcut:")) return 2;
        if (action.startsWith("cmd:")) return 3;
        if (action.startsWith("app:")) return 4;
        return 0;
    }

    /// Pick the action of a control panel button (`index` -1 adds a slot).
    function openForPanel(index: int, action: string) {
        panelIndex = index;
        dialog.trigger = "";
        tinfo = {};
        current = action;
        pending = action;
        pendingWhenRunning = "new";
        keyField.text = action.startsWith("key:") ? action.substring(4) : "";
        shortcutField.text = action.startsWith("shortcut:") ? action.substring(9) : "";
        cmdField.text = action.startsWith("cmd:") ? action.substring(4) : "";
        appId = action.startsWith("app:") ? action.substring(4) : "";
        search.text = "";
        appSearch.text = "";
        tabs.currentIndex = kindTab(action);
        rebuild();
        if (tabs.currentIndex === 4)
            loadApps();
        open();
    }

    function openFor(trigger: string, tab: int) {
        panelIndex = -2;
        dialog.trigger = trigger;
        tinfo = DaemonClient.triggerInfo(trigger);
        current = DaemonClient.actionFor(trigger);
        const b = DaemonClient.binding(trigger);
        pendingWhenRunning = b.when_running ?? "new";
        pending = current;
        keyField.text = current.startsWith("key:") ? current.substring(4) : "";
        shortcutField.text = current.startsWith("shortcut:") ? current.substring(9) : "";
        cmdField.text = current.startsWith("cmd:") ? current.substring(4) : "";
        appId = current.startsWith("app:") ? current.substring(4) : "";
        search.text = "";
        appSearch.text = "";
        tabs.currentIndex = tab > 0 ? tab : kindTab(current);
        rebuild();
        if (tabs.currentIndex === 4)
            loadApps();
        open();
    }

    function commit() {
        if (panelMode) {
            const slots = (DaemonClient.draft.panel?.controls ?? []).slice();
            if (panelIndex >= 0 && panelIndex < slots.length)
                slots[panelIndex] = "action:" + chosenAction;
            else
                slots.push("action:" + chosenAction);
            DaemonClient.setDraftValue("panel.controls", slots);
            close();
            return;
        }
        DaemonClient.setBinding(trigger, chosenAction, chosenAction.startsWith("app:") ? pendingWhenRunning : "");
        close();
    }

    function loadApps() {
        if (apps.length === 0)
            apps = Helpers.installedApps();
    }

    function rebuild() {
        actionModel.clear();
        const q = search.text.trim().toLowerCase();
        const matches = (label, action, category) => q === "" || label.toLowerCase().includes(q)
                         || action.toLowerCase().includes(q) || (category ?? "").toLowerCase().includes(q);
        const general = i18nc("@title action category", "General");
        if (panelMode) {
            // a button must do something: no "not assigned" / "system" entries
        } else if (systemOwned) {
            if (matches(i18n("System default"), "system", general))
                actionModel.append({actionId: "", labelText: i18n("System default"), category: general, iconName: "preferences-desktop-gestures-touch",
                                    detail: i18n("Pass the gesture to the desktop (KWin keeps its animation)"), slider: false});
        } else if (matches(i18n("Not assigned"), "", general)) {
            actionModel.append({actionId: "", labelText: i18n("Not assigned"), category: general, iconName: "edit-none",
                                detail: i18n("KAGeT ignores this gesture"), slider: false});
        }
        const list = DaemonClient.actions;
        for (let i = 0; i < list.length; ++i) {
            const a = list[i];
            if (a.action === "system" || (dialog.panelMode && a.action === "none"))
                continue;
            const info = DaemonClient.actionInfo(a.action);
            if (!matches(info.label, a.action, info.category))
                continue;
            let detail = a.action === "none" ? i18n("Swallow the gesture") : a.action;
            actionModel.append({actionId: a.action, labelText: info.label, category: info.category || general, iconName: info.icon,
                                detail: detail, slider: info.slider});
        }
        // Keep a custom, unlisted current action selectable.
        if (current !== "" && kindTab(current) === 0 && !list.some(a => a.action === current) && q === "") {
            const info = DaemonClient.actionInfo(current);
            actionModel.append({actionId: current, labelText: info.label, category: i18n("Current"), iconName: info.icon, detail: current, slider: info.slider});
        }
    }

    readonly property var filteredApps: {
        const q = appSearch.text.trim().toLowerCase();
        if (q === "")
            return apps;
        return apps.filter(a => a.name.toLowerCase().includes(q) || a.id.toLowerCase().includes(q)
                           || (a.genericName ?? "").toLowerCase().includes(q));
    }

    // Fixed-size content: the lists below scroll by themselves, so the
    // dialog's own ScrollView never has to.
    Item {
        implicitWidth: Kirigami.Units.gridUnit * 30
        implicitHeight: Math.max(Kirigami.Units.gridUnit * 12, Math.min((dialog.window?.height ?? 700) - Kirigami.Units.gridUnit * 11, Kirigami.Units.gridUnit * 26))

    ColumnLayout {
        id: content
        anchors.fill: parent
        spacing: 0

        QQC2.TabBar {
            id: tabs
            Layout.fillWidth: true
            onCurrentIndexChanged: if (currentIndex === 4) dialog.loadApps()
            QQC2.TabButton { text: i18nc("@title:tab", "Actions") }
            QQC2.TabButton { text: i18nc("@title:tab", "Keys") }
            QQC2.TabButton { text: i18nc("@title:tab", "Shortcut") }
            QQC2.TabButton { text: i18nc("@title:tab", "Command") }
            QQC2.TabButton { text: i18nc("@title:tab", "App") }
        }

        StackLayout {
            currentIndex: tabs.currentIndex
            Layout.fillWidth: true
            Layout.fillHeight: true

            // 0: built-in actions and KWin shortcuts
            ColumnLayout {
                spacing: 0
                Kirigami.SearchField {
                    id: search
                    Layout.fillWidth: true
                    Layout.margins: Kirigami.Units.largeSpacing
                    onTextChanged: dialog.rebuild()
                }
                QQC2.ScrollView {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.preferredHeight: Kirigami.Units.gridUnit * 4
                    ListView {
                        id: actionList
                        clip: true
                        model: ListModel {
                            id: actionModel
                        }
                        section.property: "category"
                        section.delegate: Kirigami.ListSectionHeader {
                            required property string section
                            width: ListView.view.width
                            label: section
                        }
                        delegate: QQC2.ItemDelegate {
                            id: actionDelegate
                            required property string actionId
                            required property string labelText
                            required property string iconName
                            required property string detail
                            required property bool slider
                            width: ListView.view.width
                            highlighted: dialog.pending === actionId
                            onClicked: dialog.pending = actionId
                            onDoubleClicked: {
                                dialog.pending = actionId;
                                dialog.commit();
                            }
                            contentItem: RowLayout {
                                spacing: Kirigami.Units.largeSpacing
                                Kirigami.Icon {
                                    source: actionDelegate.iconName
                                    implicitWidth: Kirigami.Units.iconSizes.smallMedium
                                    implicitHeight: Kirigami.Units.iconSizes.smallMedium
                                }
                                ColumnLayout {
                                    spacing: 0
                                    Layout.fillWidth: true
                                    QQC2.Label {
                                        text: actionDelegate.labelText
                                        elide: Text.ElideRight
                                        Layout.fillWidth: true
                                    }
                                    QQC2.Label {
                                        text: actionDelegate.detail
                                        visible: text !== "" && text !== actionDelegate.labelText
                                        font: Kirigami.Theme.smallFont
                                        opacity: 0.6
                                        elide: Text.ElideRight
                                        Layout.fillWidth: true
                                    }
                                }
                                QQC2.Label {
                                    visible: actionDelegate.slider
                                    text: i18nc("@label tag", "Slider")
                                    font: Kirigami.Theme.smallFont
                                    opacity: 0.6
                                }
                            }
                        }
                    }
                }
            }

            // 1: Linux key codes
            ColumnLayout {
                spacing: Kirigami.Units.largeSpacing
                Item { Layout.preferredHeight: Kirigami.Units.smallSpacing }
                QQC2.Label {
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.largeSpacing
                    Layout.rightMargin: Kirigami.Units.largeSpacing
                    wrapMode: Text.Wrap
                    text: i18n("Send Linux key codes, joined with “+”, for example KEY_LEFTCTRL+KEY_T. Record a combination or type it.")
                }
                RowLayout {
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.largeSpacing
                    Layout.rightMargin: Kirigami.Units.largeSpacing
                    QQC2.TextField {
                        id: keyField
                        Layout.fillWidth: true
                        placeholderText: "KEY_LEFTCTRL+KEY_T"
                        font.family: "monospace"
                        onTextEdited: text = text.toUpperCase()
                    }
                    KeyRecorder {
                        mode: "key"
                        onRecorded: (value) => keyField.text = value
                    }
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.largeSpacing
                    Layout.rightMargin: Kirigami.Units.largeSpacing
                    visible: keyField.text !== ""
                    color: Helpers.isValidKeyCombo(keyField.text.trim()) ? Kirigami.Theme.textColor : Kirigami.Theme.negativeTextColor
                    text: Helpers.isValidKeyCombo(keyField.text.trim()) ? i18n("Sends: %1", Helpers.prettyKey(keyField.text.trim()))
                                                                        : i18n("Use KEY_ names separated by “+”.")
                    wrapMode: Text.Wrap
                }
                Item { Layout.fillHeight: true }
            }

            // 2: human shortcut
            ColumnLayout {
                spacing: Kirigami.Units.largeSpacing
                Item { Layout.preferredHeight: Kirigami.Units.smallSpacing }
                QQC2.Label {
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.largeSpacing
                    Layout.rightMargin: Kirigami.Units.largeSpacing
                    wrapMode: Text.Wrap
                    text: i18n("A shortcut such as Ctrl+Alt+T. The daemon converts it to key codes using the US layout.")
                }
                RowLayout {
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.largeSpacing
                    Layout.rightMargin: Kirigami.Units.largeSpacing
                    QQC2.TextField {
                        id: shortcutField
                        Layout.fillWidth: true
                        placeholderText: "Ctrl+Alt+T"
                    }
                    KeyRecorder {
                        mode: "shortcut"
                        onRecorded: (value) => shortcutField.text = value
                    }
                }
                Item { Layout.fillHeight: true }
            }

            // 3: shell command
            ColumnLayout {
                spacing: Kirigami.Units.largeSpacing
                Item { Layout.preferredHeight: Kirigami.Units.smallSpacing }
                QQC2.Label {
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.largeSpacing
                    Layout.rightMargin: Kirigami.Units.largeSpacing
                    wrapMode: Text.Wrap
                    text: i18n("Run a shell command when the gesture is recognised.")
                }
                QQC2.TextField {
                    id: cmdField
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.largeSpacing
                    Layout.rightMargin: Kirigami.Units.largeSpacing
                    placeholderText: i18nc("@info:placeholder example command", "e.g. kcalc or notify-send Hello")
                    font.family: "monospace"
                }
                Item { Layout.fillHeight: true }
            }

            // 4: application
            ColumnLayout {
                spacing: 0
                Kirigami.SearchField {
                    id: appSearch
                    Layout.fillWidth: true
                    Layout.margins: Kirigami.Units.largeSpacing
                }
                QQC2.ScrollView {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.preferredHeight: Kirigami.Units.gridUnit * 4
                    ListView {
                        clip: true
                        model: dialog.filteredApps
                        delegate: QQC2.ItemDelegate {
                            id: appDelegate
                            required property var modelData
                            width: ListView.view.width
                            highlighted: dialog.appId === modelData.id
                            onClicked: dialog.appId = modelData.id
                            onDoubleClicked: {
                                dialog.appId = modelData.id;
                                dialog.commit();
                            }
                            contentItem: RowLayout {
                                spacing: Kirigami.Units.largeSpacing
                                Kirigami.Icon {
                                    source: appDelegate.modelData.icon
                                    implicitWidth: Kirigami.Units.iconSizes.smallMedium
                                    implicitHeight: Kirigami.Units.iconSizes.smallMedium
                                }
                                ColumnLayout {
                                    spacing: 0
                                    Layout.fillWidth: true
                                    QQC2.Label {
                                        text: appDelegate.modelData.name
                                        elide: Text.ElideRight
                                        Layout.fillWidth: true
                                    }
                                    QQC2.Label {
                                        text: appDelegate.modelData.id
                                        font: Kirigami.Theme.smallFont
                                        opacity: 0.6
                                        elide: Text.ElideRight
                                        Layout.fillWidth: true
                                    }
                                }
                            }
                        }
                        Kirigami.PlaceholderMessage {
                            anchors.centerIn: parent
                            width: parent.width - Kirigami.Units.gridUnit * 4
                            visible: parent.count === 0
                            text: i18n("No applications found")
                        }
                    }
                }
                Kirigami.Separator {
                    Layout.fillWidth: true
                }
                RowLayout {
                    // a panel button always launches (RunAction has no "if running" choice)
                    visible: !dialog.panelMode
                    Layout.fillWidth: true
                    Layout.margins: Kirigami.Units.largeSpacing
                    spacing: Kirigami.Units.largeSpacing
                    QQC2.Label {
                        text: i18n("If already running:")
                    }
                    QQC2.ComboBox {
                        Layout.fillWidth: true
                        textRole: "text"
                        valueRole: "value"
                        model: [
                            {value: "new", text: i18nc("@item:inlistbox when_running", "Launch a new instance")},
                            {value: "close", text: i18nc("@item:inlistbox when_running", "Close it")},
                            {value: "focus", text: i18nc("@item:inlistbox when_running", "Focus it")},
                            {value: "minimize", text: i18nc("@item:inlistbox when_running", "Minimize it")},
                            {value: "toggle", text: i18nc("@item:inlistbox when_running", "Toggle (focus / minimize)")}
                        ]
                        currentIndex: Math.max(0, ["new", "close", "focus", "minimize", "toggle"].indexOf(dialog.pendingWhenRunning))
                        onActivated: dialog.pendingWhenRunning = currentValue
                    }
                }
            }
        }

        Kirigami.Separator {
            Layout.fillWidth: true
        }
        RowLayout {
            Layout.fillWidth: true
            Layout.margins: Kirigami.Units.largeSpacing
            spacing: Kirigami.Units.smallSpacing
            readonly property var info: (DaemonClient.actions, DaemonClient.actionInfo(dialog.chosenAction === "" && dialog.systemOwned ? "system" : dialog.chosenAction))
            QQC2.Label {
                text: i18nc("@label", "Selected:")
                opacity: 0.7
            }
            Kirigami.Icon {
                source: parent.info.icon ?? ""
                visible: source !== ""
                implicitWidth: Kirigami.Units.iconSizes.small
                implicitHeight: Kirigami.Units.iconSizes.small
            }
            QQC2.Label {
                text: parent.info.label ?? ""
                elide: Text.ElideRight
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }
        }
    }
    }
}
