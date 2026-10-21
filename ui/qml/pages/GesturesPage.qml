// SPDX-License-Identifier: GPL-2.0-or-later
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import io.github.kaget

Kirigami.ScrollablePage {
    id: page

    title: i18nc("@title", "Gestures")
    footer: ApplyBar {}

    property string filterText: ""
    property string groupFilter: ""
    readonly property bool overrideSystem: DaemonClient.draft.general?.override_system_gestures ?? false
    readonly property bool compactRows: width < Kirigami.Units.gridUnit * 36

    readonly property var groups: [
        {value: "", text: i18nc("@item:inlistbox gesture group", "All groups")},
        {value: "corners", text: i18nc("@title gesture group", "Corners")},
        {value: "side_drag", text: i18nc("@title gesture group", "Side drag")},
        {value: "swipe", text: i18nc("@title gesture group", "Swipe")},
        {value: "pinch", text: i18nc("@title gesture group", "Pinch")},
        {value: "rotate", text: i18nc("@title gesture group", "Rotate")},
        {value: "taps", text: i18nc("@title gesture group", "Taps")}
    ]

    function setGroup(value: string) {
        groupCombo.currentIndex = Math.max(0, groups.findIndex(g => g.value === value));
        groupFilter = value;
    }

    function rebuild() {
        const q = filterText.trim().toLowerCase();
        const items = [];
        const triggers = DaemonClient.triggers;
        for (let i = 0; i < triggers.length; ++i) {
            const info = DaemonClient.triggerInfo(triggers[i].trigger);
            if (groupFilter !== "" && info.group !== groupFilter)
                continue;
            if (q !== "") {
                const action = DaemonClient.actionFor(info.trigger);
                const label = DaemonClient.actionInfo(action).label;
                const hay = (info.title + " " + info.longTitle + " " + info.trigger + " " + info.description + " " + label + " " + action).toLowerCase();
                if (!hay.includes(q))
                    continue;
            }
            const section = info.group === "corners" || info.group === "side_drag"
                ? i18nc("@title %1 group, %2 subgroup", "%1 · %2", info.groupLabel, info.subgroup)
                : info.groupLabel;
            items.push({trigger: info.trigger, section: section, order: info.groupOrder * 1000 + i});
        }
        items.sort((a, b) => a.order - b.order);
        rows.clear();
        for (const item of items)
            rows.append(item);
    }

    onFilterTextChanged: rebuild()
    onGroupFilterChanged: rebuild()
    Component.onCompleted: rebuild()

    Connections {
        target: DaemonClient
        function onTriggersChanged() {
            page.rebuild();
        }
    }

    header: QQC2.ToolBar {
        contentItem: RowLayout {
            spacing: Kirigami.Units.smallSpacing
            Kirigami.SearchField {
                Layout.fillWidth: true
                placeholderText: i18nc("@info:placeholder", "Search gestures or actions…")
                onTextChanged: page.filterText = text
            }
            QQC2.ComboBox {
                id: groupCombo
                model: page.groups
                textRole: "text"
                valueRole: "value"
                onActivated: page.groupFilter = currentValue
                Layout.preferredWidth: Kirigami.Units.gridUnit * 8
            }
        }
    }

    ListView {
        id: list
        currentIndex: -1

        model: ListModel {
            id: rows
        }

        header: ColumnLayout {
            width: list.width
            spacing: Kirigami.Units.smallSpacing

            QQC2.CheckBox {
                Layout.leftMargin: Kirigami.Units.largeSpacing
                Layout.topMargin: Kirigami.Units.largeSpacing
                Layout.fillWidth: true
                text: DaemonClient.passive ? i18n("Override system gestures (requires filter mode)") : i18n("Override system gestures")
                enabled: !DaemonClient.passive
                checked: page.overrideSystem && !DaemonClient.passive
                onToggled: DaemonClient.setDraftValue("general.override_system_gestures", checked)
            }
            Kirigami.InlineMessage {
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing
                Layout.rightMargin: Kirigami.Units.largeSpacing
                Layout.bottomMargin: Kirigami.Units.smallSpacing
                visible: true
                type: page.overrideSystem && !DaemonClient.passive ? Kirigami.MessageType.Warning : Kirigami.MessageType.Information
                text: DaemonClient.passive
                    ? i18n("Requires filter mode. The daemon runs in passive mode: it reads the touchpad alongside KDE, so system gestures (3- and 4-finger swipes and pinches, 3-finger tap) are used by KDE and cannot be rebound. Two-finger gestures are never handled by KAGeT.")
                    : page.overrideSystem
                    ? i18n("3- and 4-finger swipes and pinches and the 3-finger tap belong to KWin and libinput. Rows left at “System default” keep the desktop behaviour; any gesture you rebind replaces it, including KWin's finger-following animation.")
                    : i18n("System gestures (3- and 4-finger swipes and pinches, 3-finger tap) stay with the desktop and cannot be bound. Two-finger gestures are never handled by KAGeT.")
            }
            QQC2.Label {
                visible: rows.count === 0
                Layout.fillWidth: true
                Layout.margins: Kirigami.Units.gridUnit
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                opacity: 0.7
                text: DaemonClient.triggers.length === 0 ? i18n("No gestures reported by the daemon yet.") : i18n("No gestures match your search.")
            }
        }

        section.property: "section"
        section.delegate: Kirigami.ListSectionHeader {
            required property string section
            width: ListView.view.width
            label: section
        }

        delegate: TriggerRow {
            required property string trigger
            width: ListView.view.width
            info: (DaemonClient.triggers, DaemonClient.triggerInfo(trigger))
            compact: page.compactRows
        }
    }
}
