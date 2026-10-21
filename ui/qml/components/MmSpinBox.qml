// SPDX-License-Identifier: GPL-2.0-or-later
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import io.github.kaget

// Decimal spin box bound to one numeric value of the configuration copy.
QQC2.SpinBox {
    id: spin

    required property string path
    property int decimals: 1
    property real minimum: 0
    property real maximum: 100
    property real step: 0.5
    property string suffix: i18nc("@label unit millimetres", "mm")

    readonly property real factor: Math.pow(10, decimals)
    readonly property real current: {
        const parts = path.split(".");
        let v = DaemonClient.draft;
        for (const p of parts)
            v = v?.[p];
        return Number(v ?? 0);
    }

    from: Math.round(minimum * factor)
    // Never hide an out-of-range value coming from the daemon; it can only be lowered.
    to: Math.max(from, Math.round(Math.max(maximum, current) * factor))
    stepSize: Math.max(1, Math.round(step * factor))
    value: Math.round(current * factor)
    editable: true
    Layout.preferredWidth: Kirigami.Units.gridUnit * 7

    // Digits with an optional decimal separator and the unit suffix.
    validator: RegularExpressionValidator {
        regularExpression: /^\s*[0-9]*[.,]?[0-9]*\s*\S{0,3}\s*$/
    }

    textFromValue: function(value, locale) {
        return Number(value / factor).toLocaleString(locale, "f", decimals) + (suffix ? " " + suffix : "");
    }
    valueFromText: function(text, locale) {
        const cleaned = (suffix ? text.replace(suffix, "") : text).trim();
        try {
            const n = Number.fromLocaleString(locale, cleaned);
            return isNaN(n) ? value : Math.round(n * factor);
        } catch (e) {
            return value;
        }
    }

    onValueModified: DaemonClient.setDraftValue(path, value / factor)
}
