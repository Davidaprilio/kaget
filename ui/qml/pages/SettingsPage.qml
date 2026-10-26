// SPDX-License-Identifier: GPL-2.0-or-later
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard
import io.github.kaget

FormCard.FormCardPage {
    id: page

    title: i18nc("@title", "Settings")
    footer: ApplyBar {}

    readonly property var cfg: DaemonClient.draft
    readonly property bool numberpadSupported: DaemonClient.state.numberpad?.supported ?? false
    // the saved/draft mode decides; the daemon switches when the config is applied
    readonly property bool filterMode: (page.cfg.general?.mode ?? "passive") === "filter"
    readonly property string modeError: DaemonClient.state.mode_error ?? ""

    // ----- General -----
    FormCard.FormHeader {
        title: i18nc("@title:group", "General")
    }
    FormCard.FormCard {
        FormCard.FormSwitchDelegate {
            text: i18n("Enable gestures")
            description: i18n("Saved default. The switch on the Overview page changes it only until the daemon restarts.")
            checked: page.cfg.general?.enabled ?? true
            onToggled: DaemonClient.setDraftValue("general.enabled", checked)
        }
        FormCard.FormDelegateSeparator {}
        FormCard.FormComboBoxDelegate {
            text: i18n("Input mode")
            description: page.filterMode
                ? i18n("Filter: KAGeT takes the touchpad and passes every touch on to KDE except its own gestures. The pointer does not move during corner gestures and NumberPad keys do not click.")
                : i18n("Passive: KAGeT reads the touchpad alongside KDE. KDE sees every touch, so corner swipes also move the pointer and KDE's own gestures cannot be overridden.")
            textRole: "label"
            valueRole: "value"
            model: [
                { value: "passive", label: i18nc("@item:inlistbox input mode", "Passive") },
                { value: "filter", label: i18nc("@item:inlistbox input mode", "Filter") }
            ]
            currentIndex: page.filterMode ? 1 : 0
            onActivated: index => DaemonClient.setDraftValue("general.mode", index === 1 ? "filter" : "passive")
        }
        FormCard.FormTextDelegate {
            visible: page.modeError !== ""
            text: i18n("Filter mode is not running")
            description: i18n("The daemon fell back to passive mode: %1", page.modeError)
            icon.name: "dialog-warning"
        }
        FormCard.FormDelegateSeparator {}
        FormCard.FormSwitchDelegate {
            text: i18n("Override system gestures")
            enabled: page.filterMode
            description: !page.filterMode ? i18n("Requires filter mode. In passive mode KDE keeps its own gestures.")
                : page.cfg.general?.override_system_gestures
                ? i18n("KAGeT may take over 3- and 4-finger swipes and pinches and the 3-finger tap. Gestures you rebind lose their desktop behaviour; the rest still reach KDE.")
                : i18n("3- and 4-finger swipes and pinches and the 3-finger tap stay with KWin and libinput.")
            checked: (page.cfg.general?.override_system_gestures ?? false) && page.filterMode
            onToggled: DaemonClient.setDraftValue("general.override_system_gestures", checked)
        }
    }

    // ----- Control panel -----
    FormCard.FormHeader {
        title: i18nc("@title:group", "Control panel")
    }
    FormCard.FormCard {
        id: panelCard

        readonly property var allControls: ["brightness", "volume", "keyboard_backlight", "scale"]
        /// Slots from left to right: a built-in slider or "action:<action>" (a button).
        readonly property var slots: page.cfg.panel?.controls ?? allControls
        readonly property int maxSlots: 6
        readonly property var missingControls: allControls.filter(c => !slots.includes(c))
        readonly property bool full: slots.length >= maxSlots

        function isAction(slot) {
            return slot.startsWith("action:");
        }
        function label(slot) {
            if (isAction(slot))
                return DaemonClient.actionInfo(slot.substring(7)).label;
            switch (slot) {
            case "brightness":
                return i18n("Screen brightness");
            case "volume":
                return i18n("Volume");
            case "keyboard_backlight":
                return i18n("Keyboard backlight");
            default:
                return i18n("Panel size");
            }
        }
        function icon(slot) {
            if (isAction(slot))
                return DaemonClient.actionInfo(slot.substring(7)).icon;
            switch (slot) {
            case "brightness":
                return "brightness-high";
            case "volume":
                return "audio-volume-high";
            case "keyboard_backlight":
                return "input-keyboard-brightness";
            default:
                return "transform-scale";
            }
        }
        function setSlots(list) {
            DaemonClient.setDraftValue("panel.controls", list);
        }
        function move(i, d) {
            const list = slots.slice();
            [list[i + d], list[i]] = [list[i], list[i + d]];
            setSlots(list);
        }

        FormCard.FormTextDelegate {
            text: i18n("Slots")
            description: i18n("From left to right, as on the panel. While the panel is open the touchpad is split into the same columns: move one finger up or down in a slider's column, tap a button's column. Open and close it with a gesture bound to “Show / hide the touchpad control panel” (for example a corner hold) or with Meta+Alt+P (System Settings → Keyboard → Shortcuts → KAGeT Control Panel).")
        }

        // preview: the panel's columns in their order
        FormCard.AbstractFormDelegate {
            background: null
            contentItem: Rectangle {
                implicitHeight: Kirigami.Units.gridUnit * 4
                radius: Kirigami.Units.cornerRadius * 2
                color: Qt.alpha(Kirigami.Theme.textColor, 0.06)
                border.color: Qt.alpha(Kirigami.Theme.textColor, 0.12)

                RowLayout {
                    anchors.fill: parent
                    anchors.margins: Kirigami.Units.smallSpacing
                    spacing: Kirigami.Units.smallSpacing

                    Repeater {
                        model: panelCard.slots
                        delegate: Rectangle {
                            required property string modelData
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            radius: Kirigami.Units.cornerRadius
                            color: panelCard.isAction(modelData) ? Qt.alpha(Kirigami.Theme.highlightColor, 0.18) : Qt.alpha(Kirigami.Theme.textColor, 0.08)

                            ColumnLayout {
                                anchors.centerIn: parent
                                width: parent.width - Kirigami.Units.smallSpacing * 2
                                spacing: 2
                                Kirigami.Icon {
                                    Layout.alignment: Qt.AlignHCenter
                                    implicitWidth: Kirigami.Units.iconSizes.smallMedium
                                    implicitHeight: Kirigami.Units.iconSizes.smallMedium
                                    source: panelCard.icon(modelData)
                                }
                                QQC2.Label {
                                    Layout.fillWidth: true
                                    horizontalAlignment: Text.AlignHCenter
                                    elide: Text.ElideRight
                                    font: Kirigami.Theme.smallFont
                                    text: panelCard.label(modelData)
                                }
                            }
                        }
                    }
                }
            }
        }

        Repeater {
            model: panelCard.slots

            delegate: FormCard.AbstractFormDelegate {
                id: slotRow
                required property string modelData
                required property int index
                readonly property bool button: panelCard.isAction(modelData)

                background: null
                contentItem: RowLayout {
                    spacing: Kirigami.Units.smallSpacing
                    QQC2.Label {
                        text: slotRow.index + 1
                        opacity: 0.6
                        Layout.minimumWidth: Kirigami.Units.gridUnit
                    }
                    Kirigami.Icon {
                        source: panelCard.icon(slotRow.modelData)
                        implicitWidth: Kirigami.Units.iconSizes.small
                        implicitHeight: Kirigami.Units.iconSizes.small
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 0
                        QQC2.Label {
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                            text: panelCard.label(slotRow.modelData)
                        }
                        QQC2.Label {
                            Layout.fillWidth: true
                            font: Kirigami.Theme.smallFont
                            opacity: 0.6
                            text: slotRow.button ? i18n("Button: tap its column") : i18n("Slider: move up or down in its column")
                        }
                    }
                    QQC2.ToolButton {
                        visible: slotRow.button
                        icon.name: "document-edit"
                        display: QQC2.AbstractButton.IconOnly
                        text: i18n("Change action")
                        onClicked: applicationWindow().pickPanelAction(slotRow.index, slotRow.modelData.substring(7))
                        QQC2.ToolTip.text: text
                        QQC2.ToolTip.visible: hovered
                    }
                    QQC2.ToolButton {
                        icon.name: "arrow-left"
                        display: QQC2.AbstractButton.IconOnly
                        text: i18n("Move left")
                        enabled: slotRow.index > 0
                        onClicked: panelCard.move(slotRow.index, -1)
                        QQC2.ToolTip.text: text
                        QQC2.ToolTip.visible: hovered
                    }
                    QQC2.ToolButton {
                        icon.name: "arrow-right"
                        display: QQC2.AbstractButton.IconOnly
                        text: i18n("Move right")
                        enabled: slotRow.index < panelCard.slots.length - 1
                        onClicked: panelCard.move(slotRow.index, 1)
                        QQC2.ToolTip.text: text
                        QQC2.ToolTip.visible: hovered
                    }
                    QQC2.ToolButton {
                        icon.name: "list-remove"
                        display: QQC2.AbstractButton.IconOnly
                        text: i18n("Remove")
                        // at least one slot stays
                        enabled: panelCard.slots.length > 1
                        onClicked: panelCard.setSlots(panelCard.slots.filter((_, i) => i !== slotRow.index))
                        QQC2.ToolTip.text: text
                        QQC2.ToolTip.visible: hovered
                    }
                }
            }
        }

        FormCard.AbstractFormDelegate {
            background: null
            contentItem: RowLayout {
                spacing: Kirigami.Units.smallSpacing
                QQC2.Button {
                    id: addSlider
                    text: i18n("Add slider")
                    icon.name: "list-add"
                    enabled: panelCard.missingControls.length > 0 && !panelCard.full
                    onClicked: addMenu.popup(addSlider, 0, addSlider.height)

                    QQC2.Menu {
                        id: addMenu
                        Instantiator {
                            model: panelCard.missingControls
                            delegate: QQC2.MenuItem {
                                required property string modelData
                                text: panelCard.label(modelData)
                                icon.name: panelCard.icon(modelData)
                                onTriggered: panelCard.setSlots(panelCard.slots.concat([modelData]))
                            }
                            onObjectAdded: (index, object) => addMenu.insertItem(index, object)
                            onObjectRemoved: (index, object) => addMenu.removeItem(object)
                        }
                    }
                }
                QQC2.Button {
                    text: i18n("Add action button…")
                    icon.name: "list-add"
                    enabled: !panelCard.full
                    onClicked: applicationWindow().pickPanelAction(-1, "")
                }
                Item {
                    Layout.fillWidth: true
                }
                QQC2.Label {
                    visible: panelCard.full
                    opacity: 0.6
                    text: i18np("At most %1 slot", "At most %1 slots", panelCard.maxSlots)
                }
            }
        }
        FormCard.FormDelegateSeparator {}
        FormCard.FormComboBoxDelegate {
            text: i18n("Position")
            textRole: "label"
            valueRole: "value"
            model: [
                { value: "bottom", label: i18nc("@item:inlistbox panel position", "Bottom") },
                { value: "top", label: i18nc("@item:inlistbox panel position", "Top") },
                { value: "center", label: i18nc("@item:inlistbox panel position", "Centre") },
                { value: "bottom_left", label: i18nc("@item:inlistbox panel position", "Bottom left") },
                { value: "bottom_right", label: i18nc("@item:inlistbox panel position", "Bottom right") },
                { value: "top_left", label: i18nc("@item:inlistbox panel position", "Top left") },
                { value: "top_right", label: i18nc("@item:inlistbox panel position", "Top right") }
            ]
            currentIndex: Math.max(0, model.findIndex(m => m.value === (page.cfg.panel?.position ?? "bottom")))
            onActivated: index => DaemonClient.setDraftValue("panel.position", model[index].value)
        }
        FormCard.FormDelegateSeparator {}
        NumberDelegate {
            label: i18n("Panel size")
            description: i18n("Also changed live with the panel's own size slider.")
            path: "panel.scale"
            decimals: 2
            minimum: 0.5
            maximum: 2
            step: 0.05
            unit: "×"
        }
        FormCard.FormDelegateSeparator {}
        NumberDelegate {
            label: i18n("Close after")
            description: i18n("Seconds without slider use before the panel closes and the touchpad works normally again; 0 keeps it open until its gesture, the shortcut or Esc.")
            path: "panel.auto_hide_s"
            decimals: 1
            minimum: 0
            maximum: 60
            step: 0.5
            unit: i18nc("@label unit seconds", "s")
        }
        FormCard.FormDelegateSeparator {}
        FormCard.FormButtonDelegate {
            icon.name: "view-media-equalizer"
            text: i18n("Open the control panel")
            onClicked: DaemonClient.runAction("kaget.panel")
        }
    }

    // ----- Corners -----
    FormCard.FormHeader {
        title: i18nc("@title:group", "Corners")
    }
    FormCard.FormCard {
        NumberDelegate {
            label: i18n("Corner width")
            path: "corners.width"
            decimals: 1
            minimum: 2
            maximum: 40
            step: 0.5
            unit: i18nc("@label unit", "mm")
        }
        FormCard.FormDelegateSeparator {}
        NumberDelegate {
            label: i18n("Corner height")
            path: "corners.height"
            decimals: 1
            minimum: 2
            maximum: 40
            step: 0.5
            unit: i18nc("@label unit", "mm")
        }
        FormCard.FormDelegateSeparator {}
        NumberDelegate {
            label: i18n("Corner hold time")
            description: i18n("How long a finger must rest in a corner to trigger its hold gesture.")
            path: "corners.hold_ms"
            minimum: 50
            maximum: 3000
            step: 10
            unit: i18nc("@label unit", "ms")
        }
    }

    // ----- Side drag and sliders -----
    FormCard.FormHeader {
        title: i18nc("@title:group", "Side drag and sliders")
    }
    FormCard.FormCard {
        NumberDelegate {
            label: i18n("Side drag minimum travel")
            description: i18n("Fingers that land in a side-drag band must move inwards at least this far.")
            path: "side_drag.min_travel_mm"
            decimals: 1
            minimum: 1
            maximum: 50
            step: 0.5
            unit: i18nc("@label unit", "mm")
        }
        FormCard.FormDelegateSeparator {}
        NumberDelegate {
            label: i18n("Slider step")
            description: i18n("Distance per step for swipes bound to an action with an inverse (volume, brightness, …).")
            path: "slider.step_mm"
            decimals: 1
            minimum: 1
            maximum: 50
            step: 0.5
            unit: i18nc("@label unit", "mm")
        }
        FormCard.FormDelegateSeparator {}
        NumberDelegate {
            label: i18n("Slider continuation time")
            description: i18n("After a slider ends, a swipe along the same axis in its gesture area continues it within this time.")
            path: "slider.continue_s"
            decimals: 1
            minimum: 0
            maximum: 10
            step: 0.1
            unit: i18nc("@label unit seconds", "s")
        }
    }

    // ----- Steps -----
    FormCard.FormHeader {
        title: i18nc("@title:group", "Step sizes")
    }
    FormCard.FormCard {
        NumberDelegate {
            label: i18n("Screen brightness step")
            description: i18n("Change per step of brightness actions and sliders.")
            path: "steps.brightness_percent"
            decimals: 1
            minimum: 0.1
            maximum: 25
            step: 0.5
            unit: i18nc("@label unit percent", "%")
        }
        FormCard.FormDelegateSeparator {}
        NumberDelegate {
            label: i18n("Volume step")
            description: i18n("1 % uses Plasma's fine volume step with the on-screen indicator.")
            path: "steps.volume_percent"
            decimals: 1
            minimum: 0.1
            maximum: 25
            step: 0.5
            unit: i18nc("@label unit percent", "%")
        }
        FormCard.FormDelegateSeparator {}
        NumberDelegate {
            label: i18n("Keyboard backlight step")
            description: i18n("Keyboard backlights only have a few hardware levels, so the step is counted in levels.")
            path: "steps.keyboard_backlight_levels"
            minimum: 1
            maximum: 10
            step: 1
            unit: i18nc("@label unit hardware levels", "level(s)")
        }
    }

    // ----- Taps -----
    FormCard.FormHeader {
        title: i18nc("@title:group", "Taps and holds")
    }
    FormCard.FormCard {
        NumberDelegate {
            label: i18n("Maximum tap duration")
            path: "taps.max_tap_ms"
            minimum: 50
            maximum: 1000
            step: 10
            unit: i18nc("@label unit", "ms")
        }
        FormCard.FormDelegateSeparator {}
        NumberDelegate {
            label: i18n("Double-tap interval")
            path: "taps.double_tap_ms"
            minimum: 50
            maximum: 1500
            step: 10
            unit: i18nc("@label unit", "ms")
        }
        FormCard.FormDelegateSeparator {}
        NumberDelegate {
            label: i18n("Multi-finger hold time")
            description: i18n("Used by hold and tap-and-hold gestures with three or more fingers.")
            path: "taps.hold_ms"
            minimum: 100
            maximum: 3000
            step: 10
            unit: i18nc("@label unit", "ms")
        }
    }

    // ----- Thresholds -----
    FormCard.FormHeader {
        title: i18nc("@title:group", "Multi-finger thresholds")
    }
    FormCard.FormCard {
        NumberDelegate {
            label: i18n("Swipe distance")
            path: "gestures.swipe_mm"
            decimals: 1
            minimum: 2
            maximum: 80
            step: 0.5
            unit: i18nc("@label unit", "mm")
        }
        FormCard.FormDelegateSeparator {}
        NumberDelegate {
            label: i18n("Pinch ratio")
            description: i18n("Finger spread must shrink or grow by this factor.")
            path: "gestures.pinch_ratio"
            decimals: 2
            minimum: 1.05
            maximum: 4
            step: 0.05
            unit: "×"
        }
    }

    // ----- NumberPad -----
    FormCard.FormHeader {
        title: i18nc("@title:group", "NumberPad")
    }
    FormCard.FormCard {
        FormCard.FormSwitchDelegate {
            text: i18n("Enable the NumberPad module")
            description: page.numberpadSupported ? i18n("ASUS NumberPad detected on this touchpad.")
                                                 : i18n("No NumberPad was detected on this touchpad.")
            checked: page.cfg.numberpad?.enabled ?? false
            onToggled: DaemonClient.setDraftValue("numberpad.enabled", checked)
        }
        FormCard.FormDelegateSeparator {}
        FormCard.FormTextFieldDelegate {
            label: i18n("Calculator command")
            text: page.cfg.numberpad?.calculator_command ?? ""
            placeholderText: "kcalc"
            onTextEdited: DaemonClient.setDraftValue("numberpad.calculator_command", text)
        }
    }

    // ----- Fallback size -----
    FormCard.FormHeader {
        title: i18nc("@title:group", "Touchpad size fallback")
    }
    FormCard.FormCard {
        FormCard.FormTextDelegate {
            text: i18n("Used only when the touchpad reports no resolution.")
            textItem.wrapMode: Text.Wrap
            textItem.opacity: 0.75
        }
        NumberDelegate {
            label: i18n("Width")
            path: "device.width_mm"
            decimals: 1
            minimum: 20
            maximum: 400
            step: 0.5
            unit: i18nc("@label unit", "mm")
        }
        FormCard.FormDelegateSeparator {}
        NumberDelegate {
            label: i18n("Height")
            path: "device.height_mm"
            decimals: 1
            minimum: 20
            maximum: 300
            step: 0.5
            unit: i18nc("@label unit", "mm")
        }
    }

    // ----- Reset -----
    FormCard.FormHeader {
        title: i18nc("@title:group", "Defaults")
    }
    FormCard.FormCard {
        Layout.bottomMargin: Kirigami.Units.gridUnit
        FormCard.FormButtonDelegate {
            icon.name: "edit-reset"
            text: i18n("Reset settings to defaults")
            description: i18n("Restores every value on this page and the area lines. Your gesture bindings are kept.")
            enabled: DaemonClient.connected
            onClicked: DaemonClient.resetToDefaults(false)
        }
        FormCard.FormDelegateSeparator {}
        FormCard.FormButtonDelegate {
            icon.name: "edit-clear-history"
            text: i18n("Reset everything")
            description: i18n("Also restores the default gesture bindings.")
            enabled: DaemonClient.connected
            onClicked: DaemonClient.resetToDefaults(true)
        }
    }
}
