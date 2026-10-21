// SPDX-License-Identifier: GPL-2.0-or-later
import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard
import io.github.kaget

// FormCard spin box bound to one numeric configuration value, with decimals.
FormCard.FormSpinBoxDelegate {
    id: root

    required property string path
    property int decimals: 0
    property real minimum: 0
    property real maximum: 1000
    property real step: 1
    property string unit: ""

    readonly property real factor: Math.pow(10, decimals)
    readonly property real current: {
        const parts = path.split(".");
        let v = DaemonClient.draft;
        for (const p of parts)
            v = v?.[p];
        return Number(v ?? 0);
    }

    from: Math.round(minimum * factor)
    to: Math.round(maximum * factor)
    stepSize: Math.max(1, Math.round(step * factor))
    value: Math.round(current * factor)

    validator: RegularExpressionValidator {
        regularExpression: /^\s*[0-9]*[.,]?[0-9]*\s*\S{0,3}\s*$/
    }
    textFromValue: function(value, locale) {
        return Number(value / root.factor).toLocaleString(locale, "f", root.decimals) + (root.unit ? " " + root.unit : "");
    }
    valueFromText: function(text, locale) {
        const cleaned = (root.unit ? text.replace(root.unit, "") : text).trim();
        try {
            const n = Number.fromLocaleString(locale, cleaned);
            return isNaN(n) ? root.value : Math.round(n * root.factor);
        } catch (e) {
            return root.value;
        }
    }

    onValueModified: DaemonClient.setDraftValue(path, value / factor)
}
