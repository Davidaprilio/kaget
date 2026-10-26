// SPDX-License-Identifier: GPL-2.0-or-later
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import io.github.kaget

Kirigami.Page {
    id: page

    title: i18nc("@title:page", "Test Touchpad")

    readonly property real padW: DaemonClient.state.device?.width_mm ?? 100
    readonly property real padH: DaemonClient.state.device?.height_mm ?? 60
    readonly property bool wide: width >= Kirigami.Units.gridUnit * 44
    property bool showZones: false
    property var trails: ({})        // slot -> [{x, y}] in mm
    property var history: []         // daemon diagnoses, newest first

    function roleInfo(role: string): var {
        switch (role) {
        case "pointer": return { label: i18n("Pointer (handled by KDE)"), color: Kirigami.Theme.highlightColor };
        case "corner": return { label: i18n("Corner gesture"), color: "#9b59b6" };
        case "slider": return { label: i18n("Slider"), color: "#27ae60" };
        case "sidedrag": return { label: i18n("Side drag"), color: "#e67e22" };
        case "gesture": return { label: i18n("Multi-finger gesture"), color: "#da4453" };
        case "key": return { label: i18n("NumberPad key"), color: "#1abc9c" };
        case "withheld": return { label: i18n("Being recognised…"), color: "#f1c40f" };
        case "resting": return { label: i18n("Resting (not part of the gesture)"), color: Kirigami.Theme.disabledTextColor };
        default: return { label: role !== "" ? role : i18n("Unknown"), color: Kirigami.Theme.disabledTextColor };
        }
    }

    function summary(): string {
        const list = DaemonClient.contacts;
        if (list.length === 0)
            return i18n("No fingers on the touchpad");
        const roles = [...new Set(list.map(c => roleInfo(c.role).label))];
        return i18np("%1 finger · %2", "%1 fingers · %2", list.length, roles.join(", "));
    }

    // ---- what became of each touch (the daemon's Diagnosis signal) --------------------------------

    function entryTitle(d: var): string {
        if (d.triggerTitle)
            return d.triggerTitle;
        if (d.kind === "corner")
            return d.cornerTitle || i18n("Corner");
        return i18nc("@label gesture with %1 fingers", "%1-finger gesture", d.fingers);
    }

    function reasonText(d: var): string {
        const mm = v => Number(v).toFixed(1);
        const ms = v => Math.round(v);
        switch (d.reason) {
        case "handover": return i18n("Handed to KDE: %1", d.actionLabel || d.action);
        case "unbound": return i18n("Nothing assigned in KAGeT: left to KDE");
        case "count": return i18np("Nothing assigned for %1 finger: left to KDE", "Nothing assigned for %1 fingers: left to KDE", d.fingers);
        case "late_finger": return i18n("A finger landed %1 ms after the first (at most %2 ms): left to KDE", ms(d.late_ms), ms(d.need_ms));
        case "not_together": return i18n("The fingers landed %1 ms apart (at most %2 ms): not a gesture", ms(d.span_ms), ms(d.need_ms));
        case "busy": return i18n("A finger was still moving a slider: not a gesture");
        case "short": return i18n("Moved %1 mm, a swipe needs %2 mm", mm(d.travel_mm), mm(d.need_mm));
        case "slow_tap": return i18n("Held still for %1 ms: a tap ends within %2 ms", ms(d.duration_ms), ms(d.need_ms));
        case "short_hold": return i18n("Held still for %1 ms, a hold needs %2 ms", ms(d.duration_ms), ms(d.need_ms));
        case "tap": return i18n("A tap: KDE clicks");
        case "early": return i18n("Lifted after %1 ms, a hold needs %2 ms", ms(d.duration_ms), ms(d.need_ms));
        case "off_pad": return i18n("Swiped towards the edge of the touchpad: no corner swipe there");
        case "drifted": return i18n("Moved %1 mm while holding: too much for a hold, too little for a swipe (%2 mm)", mm(d.travel_mm), mm(d.need_mm));
        }
        return d.result === "kde" ? i18n("Left to KDE") : i18n("Not recognised");
    }

    function resultText(d: var): string {
        switch (d.result) {
        case "fired": return i18nc("@info action that ran", "→ %1", d.actionLabel || d.action);
        case "slider": return i18ncp("@info slider action and how many steps it ran", "→ %2 · %1 step", "→ %2 · %1 steps", d.steps, d.actionLabel || d.action);
        case "swallowed": return i18n("Swallowed (“Do nothing”)");
        }
        return reasonText(d);
    }

    function detailText(d: var): string {
        const parts = [Qt.formatTime(d.time, "hh:mm:ss")];
        if (d.kind === "multi" && d.span_ms > 0)
            parts.push(i18nc("@info fingers touched down within", "landed within %1 ms", Math.round(d.span_ms)));
        if (d.travel_mm > 0)
            parts.push(i18nc("@info distance", "moved %1 mm", Number(d.travel_mm).toFixed(1)));
        if (d.duration_ms > 0)
            parts.push(i18nc("@info touch duration", "%1 ms", Math.round(d.duration_ms)));
        return parts.join(" · ");
    }

    // fired / left to KDE on purpose / a near miss worth knowing about
    function statusOf(d: var): var {
        if (d.result === "fired" || d.result === "slider")
            return { icon: "dialog-ok-apply", color: Kirigami.Theme.positiveTextColor };
        if (d.result === "swallowed")
            return { icon: "dialog-cancel", color: Kirigami.Theme.disabledTextColor };
        if (["handover", "unbound", "count", "tap"].includes(d.reason))
            return { icon: "go-next", color: Kirigami.Theme.textColor };
        return { icon: "dialog-warning", color: Kirigami.Theme.neutralTextColor };
    }

    function updateTrails() {
        const t = Object.assign({}, trails);
        const present = {};
        for (const c of DaemonClient.contacts) {
            const key = String(c.slot);
            present[key] = true;
            const pts = (t[key] ?? []).slice(-40);
            pts.push({ x: c.x_mm, y: c.y_mm, role: c.role });
            t[key] = pts;
        }
        for (const key in t) {
            if (!present[key])
                delete t[key];
        }
        trails = t;
        trailCanvas.requestPaint();
    }

    Component.onCompleted: DaemonClient.setLive(true)
    Component.onDestruction: DaemonClient.setLive(false)

    Connections {
        target: DaemonClient
        function onContactsChanged() { page.updateTrails(); }
        function onGestureFired(trigger: string, action: string) { flash.restart(); }
        function onDiagnosis(d: var) { page.history = [d].concat(page.history).slice(0, 20); }
    }

    actions: [
        Kirigami.Action {
            text: i18nc("@action:intoolbar", "Show zones")
            icon.name: "view-grid"
            checkable: true
            checked: page.showZones
            onToggled: page.showZones = checked
        },
        Kirigami.Action {
            text: i18nc("@action:intoolbar", "Clear history")
            icon.name: "edit-clear-history"
            enabled: page.history.length > 0
            onTriggered: page.history = []
        }
    ]

    Kirigami.PlaceholderMessage {
        anchors.centerIn: parent
        width: parent.width - Kirigami.Units.gridUnit * 4
        visible: !DaemonClient.connected && !DaemonClient.demo
        icon.name: "io.github.kaget"
        text: i18n("The KAGeT daemon is not running")
        explanation: i18n("Touches can only be shown while kagetd is running.")
        helpfulAction: Kirigami.Action {
            text: i18n("Start daemon")
            icon.name: "media-playback-start"
            onTriggered: DaemonClient.startDaemon()
        }
    }

    GridLayout {
        anchors.fill: parent
        visible: DaemonClient.connected || DaemonClient.demo
        columns: page.wide ? 2 : 1
        columnSpacing: Kirigami.Units.largeSpacing
        rowSpacing: Kirigami.Units.largeSpacing

        // ---- touch surface ---------------------------------------------------------------
        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: Kirigami.Units.smallSpacing

            QQC2.Label {
                Layout.fillWidth: true
                text: i18n("Touch your touchpad. Every finger KAGeT sees is drawn here with how it is recognised.")
                wrapMode: Text.Wrap
                opacity: 0.8
            }

            Item {
                id: surfaceArea
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.minimumHeight: Kirigami.Units.gridUnit * 8

                readonly property real scale: Math.max(0.1, Math.min((width - 4) / page.padW, (height - 4) / page.padH))

                Rectangle {
                    id: surface
                    anchors.centerIn: parent
                    width: page.padW * surfaceArea.scale
                    height: page.padH * surfaceArea.scale
                    radius: Kirigami.Units.gridUnit
                    color: Kirigami.Theme.alternateBackgroundColor
                    border.color: flash.running ? Kirigami.Theme.positiveTextColor : Kirigami.Theme.disabledTextColor
                    border.width: flash.running ? 3 : 1
                    clip: true

                    function px(mm: real): real { return mm * surfaceArea.scale; }

                    // 1 cm grid
                    Repeater {
                        model: Math.floor(page.padW / 10)
                        Rectangle {
                            x: surface.px((index + 1) * 10)
                            width: 1
                            height: surface.height
                            color: Kirigami.Theme.textColor
                            opacity: 0.06
                        }
                    }
                    Repeater {
                        model: Math.floor(page.padH / 10)
                        Rectangle {
                            y: surface.px((index + 1) * 10)
                            height: 1
                            width: surface.width
                            color: Kirigami.Theme.textColor
                            opacity: 0.06
                        }
                    }

                    // Optional faint zones from the current config
                    Item {
                        anchors.fill: parent
                        visible: page.showZones
                        readonly property var area: DaemonClient.config.area ?? {}
                        readonly property var corners: DaemonClient.config.corners ?? {}
                        Rectangle { width: parent.width; height: surface.px(parent.area.top ?? 0); color: Kirigami.Theme.highlightColor; opacity: 0.08 }
                        Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: surface.px(parent.area.bottom ?? 0); color: Kirigami.Theme.highlightColor; opacity: 0.08 }
                        Rectangle { width: surface.px(parent.area.left ?? 0); height: parent.height; color: Kirigami.Theme.highlightColor; opacity: 0.08 }
                        Rectangle { anchors.right: parent.right; width: surface.px(parent.area.right ?? 0); height: parent.height; color: Kirigami.Theme.highlightColor; opacity: 0.08 }
                        Repeater {
                            model: 4
                            Rectangle {
                                width: surface.px(parent.corners.width ?? 8)
                                height: surface.px(parent.corners.height ?? 10)
                                x: index % 2 === 0 ? 0 : surface.width - width
                                y: index < 2 ? 0 : surface.height - height
                                color: "#9b59b6"
                                opacity: 0.25
                            }
                        }
                    }

                    QQC2.Label {
                        anchors.centerIn: parent
                        visible: DaemonClient.contacts.length === 0
                        text: i18n("Waiting for touches…")
                        opacity: 0.5
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.3
                    }

                    // Movement trails
                    Canvas {
                        id: trailCanvas
                        anchors.fill: parent
                        onPaint: {
                            const ctx = getContext("2d");
                            ctx.reset();
                            ctx.lineWidth = 3;
                            ctx.lineCap = "round";
                            for (const key in page.trails) {
                                const pts = page.trails[key];
                                for (let i = 1; i < pts.length; ++i) {
                                    ctx.globalAlpha = i / pts.length * 0.6;
                                    ctx.strokeStyle = page.roleInfo(pts[i].role).color;
                                    ctx.beginPath();
                                    ctx.moveTo(surface.px(pts[i - 1].x), surface.px(pts[i - 1].y));
                                    ctx.lineTo(surface.px(pts[i].x), surface.px(pts[i].y));
                                    ctx.stroke();
                                }
                            }
                        }
                    }

                    // Fingers
                    Repeater {
                        model: DaemonClient.contacts
                        delegate: Item {
                            required property var modelData
                            readonly property var info: page.roleInfo(modelData.role)
                            x: surface.px(modelData.x_mm) - width / 2
                            y: surface.px(modelData.y_mm) - height / 2
                            width: Kirigami.Units.gridUnit * 2.4
                            height: width

                            Rectangle {
                                anchors.fill: parent
                                radius: width / 2
                                color: parent.info.color
                                opacity: 0.35
                            }
                            Rectangle {
                                anchors.centerIn: parent
                                width: parent.width * 0.55
                                height: width
                                radius: width / 2
                                color: parent.info.color
                                QQC2.Label {
                                    anchors.centerIn: parent
                                    text: String(modelData.slot + 1)
                                    color: "white"
                                    font.bold: true
                                }
                            }
                            QQC2.Label {
                                anchors.top: parent.bottom
                                anchors.horizontalCenter: parent.horizontalCenter
                                // With several fingers the labels would overlap; the side panel lists them.
                                visible: DaemonClient.contacts.length === 1
                                text: parent.info.label
                                font: Kirigami.Theme.smallFont
                                color: parent.info.color
                            }
                        }
                    }
                }
            }
        }

        // ---- detected input panel -------------------------------------------------------
        ColumnLayout {
            Layout.preferredWidth: page.wide ? Kirigami.Units.gridUnit * 18 : -1
            Layout.fillWidth: !page.wide
            Layout.fillHeight: page.wide
            Layout.maximumHeight: page.wide ? -1 : Kirigami.Units.gridUnit * 16
            spacing: Kirigami.Units.smallSpacing

            Kirigami.Heading {
                level: 3
                text: i18n("Detected input")
            }
            QQC2.Label {
                Layout.fillWidth: true
                text: page.summary()
                wrapMode: Text.Wrap
                font.bold: DaemonClient.contacts.length > 0
            }

            Repeater {
                model: DaemonClient.contacts
                delegate: RowLayout {
                    required property var modelData
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing
                    Rectangle {
                        implicitWidth: Kirigami.Units.gridUnit * 0.8
                        implicitHeight: implicitWidth
                        radius: width / 2
                        color: page.roleInfo(modelData.role).color
                    }
                    QQC2.Label {
                        Layout.fillWidth: true
                        text: i18nc("@info finger %1, x %2 mm, y %3 mm, role %4", "Finger %1 · %2, %3 mm · %4",
                                    modelData.slot + 1, Number(modelData.x_mm).toFixed(1),
                                    Number(modelData.y_mm).toFixed(1), page.roleInfo(modelData.role).label)
                        elide: Text.ElideRight
                    }
                }
            }

            Kirigami.Separator { Layout.fillWidth: true }

            Kirigami.Heading {
                level: 3
                text: i18n("Last gesture")
            }
            Rectangle {
                Layout.fillWidth: true
                implicitHeight: lastRow.implicitHeight + Kirigami.Units.largeSpacing * 2
                radius: Kirigami.Units.smallSpacing
                color: flash.running ? Qt.alpha(Kirigami.Theme.positiveTextColor, 0.2) : Kirigami.Theme.alternateBackgroundColor
                Behavior on color { ColorAnimation { duration: Kirigami.Units.longDuration } }
                RowLayout {
                    id: lastRow
                    readonly property var entry: page.history.length > 0 ? page.history[0] : null
                    anchors.fill: parent
                    anchors.margins: Kirigami.Units.largeSpacing
                    spacing: Kirigami.Units.largeSpacing
                    Kirigami.Icon {
                        Layout.alignment: Qt.AlignTop
                        implicitWidth: Kirigami.Units.iconSizes.medium
                        implicitHeight: implicitWidth
                        visible: lastRow.entry !== null
                        source: lastRow.entry ? page.statusOf(lastRow.entry).icon : ""
                        color: lastRow.entry ? page.statusOf(lastRow.entry).color : "transparent"
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 0
                        QQC2.Label {
                            Layout.fillWidth: true
                            text: lastRow.entry ? page.entryTitle(lastRow.entry) : i18n("No gesture recognised yet")
                            wrapMode: Text.Wrap
                            font.bold: lastRow.entry !== null
                        }
                        QQC2.Label {
                            Layout.fillWidth: true
                            visible: lastRow.entry !== null
                            text: lastRow.entry ? page.resultText(lastRow.entry) : ""
                            wrapMode: Text.Wrap
                        }
                        QQC2.Label {
                            Layout.fillWidth: true
                            visible: lastRow.entry !== null
                            text: lastRow.entry ? page.detailText(lastRow.entry) : ""
                            wrapMode: Text.Wrap
                            font: Kirigami.Theme.smallFont
                            opacity: 0.7
                        }
                    }
                }
            }

            QQC2.Label {
                text: i18n("History")
                font.bold: true
                visible: page.history.length > 1
            }
            QQC2.ScrollView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: page.history.length > 1
                ListView {
                    model: page.history.slice(1)
                    clip: true
                    delegate: QQC2.ItemDelegate {
                        required property var modelData
                        width: ListView.view.width
                        contentItem: RowLayout {
                            spacing: Kirigami.Units.smallSpacing
                            Kirigami.Icon {
                                Layout.alignment: Qt.AlignTop
                                implicitWidth: Kirigami.Units.iconSizes.small
                                implicitHeight: implicitWidth
                                source: page.statusOf(modelData).icon
                                color: page.statusOf(modelData).color
                            }
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 0
                                QQC2.Label {
                                    Layout.fillWidth: true
                                    text: page.entryTitle(modelData)
                                    elide: Text.ElideRight
                                }
                                QQC2.Label {
                                    Layout.fillWidth: true
                                    text: page.resultText(modelData)
                                    wrapMode: Text.Wrap
                                    font: Kirigami.Theme.smallFont
                                }
                                QQC2.Label {
                                    Layout.fillWidth: true
                                    text: page.detailText(modelData)
                                    font: Kirigami.Theme.smallFont
                                    opacity: 0.7
                                    wrapMode: Text.Wrap
                                }
                            }
                        }
                    }
                }
            }
            Item { Layout.fillHeight: true; visible: page.history.length <= 1 }
        }
    }

    Timer {
        id: flash
        interval: 600
    }
}
