// SPDX-License-Identifier: GPL-2.0-or-later
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard

// Label / value row for FormCards (value is selectable, wraps).
FormCard.AbstractFormDelegate {
    id: row

    property string label: ""
    property string value: ""
    property string iconName: ""
    property color valueColor: Kirigami.Theme.textColor
    property bool monospace: false

    background: null
    hoverEnabled: false

    contentItem: RowLayout {
        spacing: Kirigami.Units.largeSpacing

        Kirigami.Icon {
            visible: row.iconName !== ""
            source: row.iconName
            implicitWidth: Kirigami.Units.iconSizes.smallMedium
            implicitHeight: Kirigami.Units.iconSizes.smallMedium
            Layout.alignment: Qt.AlignTop
        }
        QQC2.Label {
            text: row.label
            Layout.preferredWidth: Math.min(implicitWidth, row.width * 0.4)
            Layout.alignment: Qt.AlignTop
            elide: Text.ElideRight
            opacity: 0.8
        }
        Kirigami.SelectableLabel {
            text: row.value
            color: row.valueColor
            wrapMode: Text.Wrap
            horizontalAlignment: Text.AlignRight
            font.family: row.monospace ? "monospace" : Kirigami.Theme.defaultFont.family
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignTop
        }
    }
}
