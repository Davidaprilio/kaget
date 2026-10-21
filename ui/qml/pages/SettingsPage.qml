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
        FormCard.FormDelegateSeparator {}
        NumberDelegate {
            label: i18n("Rotation angle")
            path: "gestures.rotate_deg"
            minimum: 5
            maximum: 180
            step: 1
            unit: "°"
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
