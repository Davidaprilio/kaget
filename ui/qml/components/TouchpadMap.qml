// SPDX-License-Identifier: GPL-2.0-or-later
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Shapes
import org.kde.kirigami as Kirigami
import io.github.kaget

// The touchpad drawn with its real aspect ratio. Coordinates are millimetres
// with the origin at the top-left corner; `s` converts mm to pixels.
Item {
    id: map

    // Self reference for children whose own `map` property shadows the id.
    readonly property Item mapItem: map
    readonly property Item padItem: pad

    // ----- inputs -----
    property var cfg: DaemonClient.draft
    property var deviceInfo: DaemonClient.state.device ?? ({})
    property var numberpadInfo: DaemonClient.state.numberpad ?? ({})
    property bool interactive: true
    property bool showLabels: true
    property bool showNumberPad: false
    property bool live: false
    property string selectedZone: ""
    property string previewTrigger: ""
    property bool freezePreview: false
    property string forcedReadout: ""

    signal zoneActivated(string zone)
    signal triggerActivated(string trigger)

    // ----- geometry (mm) -----
    readonly property real widthMm: (deviceInfo.width_mm ?? 0) > 0 ? deviceInfo.width_mm : (cfg.device?.width_mm ?? 100)
    readonly property real heightMm: (deviceInfo.height_mm ?? 0) > 0 ? deviceInfo.height_mm : (cfg.device?.height_mm ?? 60)
    readonly property real aspect: widthMm / heightMm
    readonly property int maxFingers: deviceInfo.max_fingers ?? 5

    // Feature groups that can be switched off from the legend; disabled groups are not drawn.
    readonly property bool areaOn: cfg.area?.enabled ?? true
    readonly property bool sideOn: cfg.side_drag?.enabled ?? false
    readonly property bool cornersOn: cfg.corners?.enabled ?? true
    readonly property real areaTop: areaOn ? (cfg.area?.top ?? 0) : 0
    readonly property real areaRight: areaOn ? (cfg.area?.right ?? 0) : 0
    readonly property real areaBottom: areaOn ? (cfg.area?.bottom ?? 0) : 0
    readonly property real areaLeft: areaOn ? (cfg.area?.left ?? 0) : 0
    readonly property real sideTop: sideOn ? (cfg.side_drag?.top ?? 0) : 0
    readonly property real sideRight: sideOn ? (cfg.side_drag?.right ?? 0) : 0
    readonly property real sideBottom: sideOn ? (cfg.side_drag?.bottom ?? 0) : 0
    readonly property real sideLeft: sideOn ? (cfg.side_drag?.left ?? 0) : 0
    readonly property real cornerW: cfg.corners?.width ?? 8
    readonly property real cornerH: cfg.corners?.height ?? 10
    readonly property bool overrideSystem: (cfg.general?.override_system_gestures ?? false) && !DaemonClient.passive
    readonly property var sideDragFingers: {
        const out = [];
        for (let n = 3; n <= Math.min(4, maxFingers); ++n)
            out.push(n);
        return out;
    }

    // ----- scale (px per mm) -----
    readonly property real s: Math.max(0.01, Math.min(width / widthMm, height / heightMm))
    readonly property real padW: widthMm * s
    readonly property real padH: heightMm * s
    readonly property bool roomy: padW > Kirigami.Units.gridUnit * 22

    implicitWidth: Kirigami.Units.gridUnit * 32
    implicitHeight: implicitWidth / aspect

    // ----- colours -----
    Kirigami.Theme.colorSet: Kirigami.Theme.View
    Kirigami.Theme.inherit: false
    readonly property bool dark: Kirigami.ColorUtils.brightnessForColor(Kirigami.Theme.backgroundColor) === Kirigami.ColorUtils.Dark
    readonly property real tint: dark ? 1.7 : 1.0
    readonly property color areaColor: Kirigami.Theme.highlightColor
    readonly property color sideColor: Kirigami.Theme.neutralTextColor
    readonly property color cornerColor: dark ? "#b57edc" : "#9b59b6"
    readonly property color fingerColor: Kirigami.Theme.positiveTextColor
    readonly property color padColor: Kirigami.ColorUtils.tintWithAlpha(Kirigami.Theme.backgroundColor, Kirigami.Theme.textColor, 0.05)
    readonly property color lineTextColor: Kirigami.Theme.textColor

    function alpha(c, a) {
        return Qt.rgba(c.r, c.g, c.b, a);
    }

    function roleColor(role) {
        switch (role) {
        case "key": return map.cornerColor;
        case "corner": return map.cornerColor;
        case "slider": return map.areaColor;
        case "sidedrag": return map.sideColor;
        case "gesture": return map.fingerColor;
        case "withheld": return Kirigami.Theme.disabledTextColor;
        default: return Kirigami.ColorUtils.linearInterpolation(Kirigami.Theme.textColor, Kirigami.Theme.backgroundColor, 0.35);
        }
    }

    // ----- bindings helpers -----
    function actionOf(trigger) {
        const list = cfg.bindings ?? [];
        for (let i = 0; i < list.length; ++i) {
            if (list[i].trigger === trigger)
                return list[i].action;
        }
        return "";
    }

    function bindingsWithPrefix(prefix) {
        const list = cfg.bindings ?? [];
        return list.filter(b => b.trigger.startsWith(prefix));
    }

    function slidersInBand(edge) {
        const out = [];
        const list = cfg.bindings ?? [];
        for (let i = 0; i < list.length; ++i) {
            const p = list[i].trigger.split(".");
            if (p[0] !== "corner" || p[2] !== "swipe")
                continue;
            const corner = p[1], dir = p[3];
            const info = DaemonClient.actionInfo(list[i].action);
            if (!info.slider)
                continue;
            const horizontal = dir === "left" || dir === "right";
            const vertical = dir === "up" || dir === "down";
            if ((edge === "top" && horizontal)
                    || (edge === "right" && vertical && corner.endsWith("right"))
                    || (edge === "left" && vertical && corner.endsWith("left"))) {
                if (!out.includes(info.label))
                    out.push(info.label);
            }
        }
        return out;
    }

    // ----- zones -----
    function zoneAt(x, y) {
        const W = widthMm, H = heightMm;
        if (y < cornerH && x < cornerW) return "corner.top_left";
        if (y < cornerH && x > W - cornerW) return "corner.top_right";
        if (y > H - cornerH && x < cornerW) return "corner.bottom_left";
        if (y > H - cornerH && x > W - cornerW) return "corner.bottom_right";
        if (sideTop > 0 && y < sideTop) return "side.top";
        if (sideBottom > 0 && y > H - sideBottom) return "side.bottom";
        if (sideLeft > 0 && x < sideLeft) return "side.left";
        if (sideRight > 0 && x > W - sideRight) return "side.right";
        if (y < areaTop) return "band.top";
        if (y > H - areaBottom) return "band.bottom";
        if (x < areaLeft) return "band.left";
        if (x > W - areaRight) return "band.right";
        return "centre";
    }

    function zoneRect(zone) {
        const W = widthMm, H = heightMm;
        switch (zone) {
        case "corner.top_left": return Qt.rect(0, 0, cornerW, cornerH);
        case "corner.top_right": return Qt.rect(W - cornerW, 0, cornerW, cornerH);
        case "corner.bottom_left": return Qt.rect(0, H - cornerH, cornerW, cornerH);
        case "corner.bottom_right": return Qt.rect(W - cornerW, H - cornerH, cornerW, cornerH);
        case "side.top": return Qt.rect(cornerW, 0, W - 2 * cornerW, sideTop);
        case "side.bottom": return Qt.rect(cornerW, H - sideBottom, W - 2 * cornerW, sideBottom);
        case "side.left": return Qt.rect(0, cornerH, sideLeft, H - 2 * cornerH);
        case "side.right": return Qt.rect(W - sideRight, cornerH, sideRight, H - 2 * cornerH);
        case "band.top": return Qt.rect(0, 0, W, areaTop);
        case "band.bottom": return Qt.rect(0, H - areaBottom, W, areaBottom);
        case "band.left": return Qt.rect(0, areaTop, areaLeft, H - areaTop - areaBottom);
        case "band.right": return Qt.rect(W - areaRight, areaTop, areaRight, H - areaTop - areaBottom);
        case "centre": return Qt.rect(areaLeft, areaTop, W - areaLeft - areaRight, H - areaTop - areaBottom);
        }
        return Qt.rect(0, 0, 0, 0);
    }

    readonly property var liveZones: {
        if (!live)
            return [];
        const out = [];
        const contacts = DaemonClient.contacts;
        for (let i = 0; i < contacts.length; ++i) {
            const z = zoneAt(contacts[i].x_mm, contacts[i].y_mm);
            if (!out.includes(z))
                out.push(z);
        }
        return out;
    }
    property string hoveredZone: ""

    function zoneEmphasis(zone) {
        if (liveZones.includes(zone))
            return 2.4;
        if (selectedZone === zone)
            return 1.9;
        if (hoveredZone === zone)
            return 1.5;
        return 1.0;
    }

    // Maximum for a line so that the centre keeps >= 50 %.
    function maxFor(section, edge) {
        const opposite = {top: "bottom", bottom: "top", left: "right", right: "left"}[edge];
        const other = cfg[section]?.[opposite] ?? 0;
        const total = (edge === "top" || edge === "bottom") ? heightMm : widthMm;
        return Math.max(0, Math.floor((total * 0.5 - other) * 10) / 10);
    }

    // ===================================================================
    Rectangle {
        id: pad
        x: Math.round((map.width - map.padW) / 2)
        y: Math.round((map.height - map.padH) / 2)
        width: map.padW
        height: map.padH
        radius: Math.min(5 * map.s, Kirigami.Units.gridUnit)
        color: map.padColor
        border.width: 1
        border.color: Kirigami.ColorUtils.linearInterpolation(Kirigami.Theme.backgroundColor, Kirigami.Theme.textColor, 0.35)

        // ----- area bands -----
        Rectangle {
            visible: map.areaTop > 0
            width: pad.width
            height: map.areaTop * map.s
            topLeftRadius: pad.radius
            topRightRadius: pad.radius
            color: map.alpha(map.areaColor, 0.09 * map.tint * map.zoneEmphasis("band.top"))
        }
        Rectangle {
            visible: map.areaBottom > 0
            y: pad.height - height
            width: pad.width
            height: map.areaBottom * map.s
            bottomLeftRadius: pad.radius
            bottomRightRadius: pad.radius
            color: map.alpha(map.areaColor, 0.09 * map.tint * map.zoneEmphasis("band.bottom"))
        }
        Rectangle {
            visible: map.areaLeft > 0
            y: map.areaTop * map.s
            width: map.areaLeft * map.s
            height: pad.height - (map.areaTop + map.areaBottom) * map.s
            topLeftRadius: map.areaTop > 0 ? 0 : pad.radius
            bottomLeftRadius: map.areaBottom > 0 ? 0 : pad.radius
            color: map.alpha(map.areaColor, 0.09 * map.tint * map.zoneEmphasis("band.left"))
        }
        Rectangle {
            visible: map.areaRight > 0
            x: pad.width - width
            y: map.areaTop * map.s
            width: map.areaRight * map.s
            height: pad.height - (map.areaTop + map.areaBottom) * map.s
            topRightRadius: map.areaTop > 0 ? 0 : pad.radius
            bottomRightRadius: map.areaBottom > 0 ? 0 : pad.radius
            color: map.alpha(map.areaColor, 0.09 * map.tint * map.zoneEmphasis("band.right"))
        }

        // ----- side-drag bands with inward chevrons -----
        Repeater {
            model: ["top", "right", "bottom", "left"]
            delegate: Item {
                id: sideBand
                required property string modelData
                readonly property string edge: modelData
                readonly property bool horizontal: edge === "top" || edge === "bottom"
                readonly property real depth: (edge === "top" ? map.sideTop : edge === "right" ? map.sideRight
                                               : edge === "bottom" ? map.sideBottom : map.sideLeft) * map.s
                visible: depth > 0
                x: edge === "right" ? pad.width - depth : (horizontal ? map.cornerW * map.s : 0)
                y: edge === "bottom" ? pad.height - depth : (horizontal ? 0 : map.cornerH * map.s)
                width: horizontal ? pad.width - 2 * map.cornerW * map.s : depth
                height: horizontal ? depth : pad.height - 2 * map.cornerH * map.s

                Rectangle {
                    anchors.fill: parent
                    color: map.alpha(map.sideColor, 0.16 * map.tint * map.zoneEmphasis("side." + sideBand.edge))
                }

                Repeater {
                    model: Math.max(0, Math.floor((sideBand.horizontal ? sideBand.width : sideBand.height) / Math.max(1, 11 * map.s)))
                    delegate: Shape {
                        id: chevron
                        required property int index
                        readonly property real size: Math.max(4, Math.min(sideBand.depth * 0.55, 9))
                        readonly property real along: (index + 0.5) * Math.max(1, 11 * map.s)
                        width: size
                        height: size
                        x: sideBand.horizontal ? along - size / 2 : (sideBand.width - size) / 2
                        y: sideBand.horizontal ? (sideBand.height - size) / 2 : along - size / 2
                        rotation: sideBand.edge === "top" ? 0 : sideBand.edge === "bottom" ? 180 : sideBand.edge === "left" ? -90 : 90
                        opacity: 0.55
                        preferredRendererType: Shape.CurveRenderer
                        ShapePath {
                            strokeColor: map.sideColor
                            strokeWidth: 1.5
                            fillColor: "transparent"
                            capStyle: ShapePath.RoundCap
                            joinStyle: ShapePath.RoundJoin
                            PathPolyline {
                                path: [Qt.point(0, chevron.size * 0.3), Qt.point(chevron.size / 2, chevron.size * 0.75), Qt.point(chevron.size, chevron.size * 0.3)]
                            }
                        }
                    }
                }
            }
        }

        // ----- corners -----
        Repeater {
            model: [
                {id: "top_left", left: true, top: true},
                {id: "top_right", left: false, top: true},
                {id: "bottom_left", left: true, top: false},
                {id: "bottom_right", left: false, top: false}
            ]
            delegate: Rectangle {
                id: cornerRect
                required property var modelData
                visible: map.cornersOn
                readonly property string zone: "corner." + modelData.id
                readonly property string holdAction: map.actionOf(zone + ".hold")
                readonly property var swipes: map.bindingsWithPrefix(zone + ".swipe.")
                readonly property string iconAction: holdAction !== "" ? holdAction : (swipes.length > 0 ? swipes[0].action : "")
                readonly property var iconInfo: (DaemonClient.actions, DaemonClient.actionInfo(iconAction))

                x: modelData.left ? 0 : pad.width - width
                y: modelData.top ? 0 : pad.height - height
                width: map.cornerW * map.s
                height: map.cornerH * map.s
                topLeftRadius: modelData.left && modelData.top ? pad.radius : 2
                topRightRadius: !modelData.left && modelData.top ? pad.radius : 2
                bottomLeftRadius: modelData.left && !modelData.top ? pad.radius : 2
                bottomRightRadius: !modelData.left && !modelData.top ? pad.radius : 2
                color: map.alpha(map.cornerColor, 0.2 * map.tint * map.zoneEmphasis(zone))
                border.width: 1
                border.color: map.alpha(map.cornerColor, 0.8)

                Kirigami.Icon {
                    anchors.centerIn: parent
                    readonly property real size: Math.max(10, Math.min(Kirigami.Units.iconSizes.medium, Math.min(parent.width, parent.height) * 0.72))
                    width: size
                    height: size
                    visible: cornerRect.iconAction !== ""
                    source: cornerRect.iconInfo.icon ?? ""
                }
            }
        }

        // ----- NumberPad key grid -----
        Repeater {
            model: map.showNumberPad && (map.numberpadInfo.supported ?? false) ? (map.numberpadInfo.keys ?? []) : []
            delegate: Rectangle {
                id: key
                required property var modelData
                x: modelData.x0 * map.s + 1
                y: modelData.y0 * map.s + 1
                width: (modelData.x1 - modelData.x0) * map.s - 2
                height: (modelData.y1 - modelData.y0) * map.s - 2
                radius: 3
                color: map.alpha(Kirigami.Theme.backgroundColor, (map.numberpadInfo.active ?? false) ? 0.75 : 0.6)
                border.width: 1
                border.color: map.alpha(Kirigami.Theme.textColor, 0.35)
                QQC2.Label {
                    anchors.centerIn: parent
                    text: key.modelData.label
                    font.pixelSize: Math.max(9, Math.min(key.width, key.height) * 0.42)
                    color: Kirigami.Theme.textColor
                    opacity: 0.8
                }
            }
        }

        // ----- captions -----
        // Centre: multi-finger gestures summary.
        Column {
            id: centreCaption
            readonly property rect r: map.zoneRect("centre")
            visible: map.showLabels && !map.showNumberPad && map.roomy && r.width * map.s > 120 && r.height * map.s > 60
            x: (r.x + r.width / 2) * map.s - width / 2
            y: (r.y + r.height / 2) * map.s - height / 2
            width: Math.min(r.width * map.s - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 18)
            spacing: 2
            readonly property int bound: (map.cfg.bindings ?? []).filter(b => /^(swipe|pinch|hold|tap|doubletap|taphold)\./.test(b.trigger)).length

            QQC2.Label {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                text: i18n("Multi-finger gestures")
                font.weight: Font.DemiBold
                elide: Text.ElideRight
                opacity: 0.8
            }
            QQC2.Label {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                text: centreCaption.bound > 0 ? i18np("%1 gesture bound", "%1 gestures bound", centreCaption.bound) : i18n("No custom gestures yet")
                font: Kirigami.Theme.smallFont
                elide: Text.ElideRight
                opacity: 0.7
            }
            QQC2.Label {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                maximumLineCount: 2
                elide: Text.ElideRight
                text: map.overrideSystem ? i18n("System gestures can be rebound") : i18n("System: 3/4-finger swipe and pinch, 3-finger tap")
                font: Kirigami.Theme.smallFont
                color: map.overrideSystem ? Kirigami.Theme.textColor : Kirigami.Theme.disabledTextColor
                opacity: map.overrideSystem ? 0.7 : 1
            }
        }

        // Area bands: which sliders continue there.
        Repeater {
            model: ["top", "left", "right"]
            delegate: QQC2.Label {
                id: bandCaption
                required property string modelData
                readonly property var sliders: map.slidersInBand(modelData)
                readonly property real bandMm: modelData === "top" ? map.areaTop : modelData === "left" ? map.areaLeft : map.areaRight
                readonly property real sideMm: modelData === "top" ? map.sideTop : modelData === "left" ? map.sideLeft : map.sideRight
                visible: map.cornersOn && map.showLabels && map.roomy && !map.showNumberPad && sliders.length > 0 && (bandMm - sideMm) * map.s > Kirigami.Units.gridUnit * 2.5
                text: i18nc("@info:tooltip %1 list of slider actions", "↔ Slider continues: %1", sliders.join(", "))
                font: Kirigami.Theme.smallFont
                opacity: 0.75
                wrapMode: Text.Wrap
                maximumLineCount: 3
                elide: Text.ElideRight
                horizontalAlignment: modelData === "top" ? Text.AlignHCenter : (modelData === "left" ? Text.AlignLeft : Text.AlignRight)
                width: modelData === "top" ? Math.min(pad.width * 0.5, Kirigami.Units.gridUnit * 18)
                                           : Math.max(Kirigami.Units.gridUnit * 3, (bandMm - sideMm) * map.s - Kirigami.Units.largeSpacing)
                x: modelData === "top" ? (pad.width - width) / 2
                                       : modelData === "left" ? map.sideLeft * map.s + Kirigami.Units.smallSpacing
                                                              : pad.width - map.sideRight * map.s - width - Kirigami.Units.smallSpacing
                y: modelData === "top" ? map.areaTop * map.s - height - Kirigami.Units.smallSpacing
                                       : pad.height * 0.64
            }
        }

        // ----- zone hit testing -----
        MouseArea {
            id: zoneMouse
            anchors.fill: parent
            enabled: map.interactive
            hoverEnabled: map.interactive
            cursorShape: Qt.PointingHandCursor
            onPositionChanged: (mouse) => map.hoveredZone = map.zoneAt(mouse.x / map.s, mouse.y / map.s)
            onExited: map.hoveredZone = ""
            onClicked: (mouse) => map.zoneActivated(map.zoneAt(mouse.x / map.s, mouse.y / map.s))
        }

        // ----- corner callouts -----
        Repeater {
            model: [
                {id: "top_left", left: true, top: true},
                {id: "top_right", left: false, top: true},
                {id: "bottom_left", left: true, top: false},
                {id: "bottom_right", left: false, top: false}
            ]
            delegate: Item {
                id: callout
                required property var modelData
                readonly property string zone: "corner." + modelData.id
                readonly property string holdAction: map.actionOf(zone + ".hold")
                readonly property int swipeCount: map.bindingsWithPrefix(zone + ".swipe.").length
                readonly property real bandMm: modelData.left ? map.areaLeft : map.areaRight
                visible: map.cornersOn && map.showLabels && map.roomy && !map.showNumberPad && (holdAction !== "" || swipeCount > 0)
                width: Math.max(Kirigami.Units.gridUnit * 4, Math.min(Math.max(bandMm, 18) * map.s, map.padW * 0.3) - Kirigami.Units.smallSpacing * 2)
                x: modelData.left ? Kirigami.Units.smallSpacing + 2 : pad.width - width - Kirigami.Units.smallSpacing - 2
                y: modelData.top ? map.cornerH * map.s + Kirigami.Units.smallSpacing : pad.height - map.cornerH * map.s - height - Kirigami.Units.smallSpacing
                height: calloutColumn.implicitHeight

                Column {
                id: calloutColumn
                width: parent.width
                spacing: 0

                QQC2.Label {
                    width: parent.width
                    visible: callout.holdAction !== ""
                    horizontalAlignment: callout.modelData.left ? Text.AlignLeft : Text.AlignRight
                    text: i18nc("@label corner hold, %1 action", "Hold: %1", (DaemonClient.actions, DaemonClient.actionInfo(callout.holdAction).label))
                    font: Kirigami.Theme.smallFont
                    elide: Text.ElideRight
                }
                QQC2.Label {
                    width: parent.width
                    visible: callout.swipeCount > 0
                    horizontalAlignment: callout.modelData.left ? Text.AlignLeft : Text.AlignRight
                    text: i18np("+%1 swipe", "+%1 swipes", callout.swipeCount)
                    font: Kirigami.Theme.smallFont
                    opacity: 0.7
                    elide: Text.ElideRight
                }
                }
                MouseArea {
                    // Click on the callout selects its corner.
                    anchors.fill: parent
                    enabled: map.interactive
                    cursorShape: Qt.PointingHandCursor
                    onClicked: map.zoneActivated(callout.zone)
                }
            }
        }

        // ----- side-drag badges ("3F ↓ Overview") -----
        Repeater {
            model: ["top", "right", "bottom", "left"]
            delegate: SideDragBadges {
                required property string modelData
                edge: modelData
                map: mapItem
                visible: map.showLabels && depthMm > 0
            }
        }

        // ----- selection / hover outline -----
        Rectangle {
            readonly property rect r: map.zoneRect(map.selectedZone)
            visible: map.selectedZone !== "" && r.width > 0 && r.height > 0
            x: r.x * map.s
            y: r.y * map.s
            width: r.width * map.s
            height: r.height * map.s
            radius: 3
            color: "transparent"
            border.width: 2
            border.color: Kirigami.Theme.focusColor
        }

        // ----- draggable lines -----
        Repeater {
            model: map.interactive || map.showLabels ? [
                {section: "area", edge: "top", grip: 0.5},
                {section: "area", edge: "right", grip: 0.5},
                {section: "area", edge: "bottom", grip: 0.5},
                {section: "area", edge: "left", grip: 0.5},
                {section: "side_drag", edge: "top", grip: 0.82},
                {section: "side_drag", edge: "right", grip: 0.78},
                {section: "side_drag", edge: "bottom", grip: 0.82},
                {section: "side_drag", edge: "left", grip: 0.78}
            ] : []
            delegate: DragLine {
                required property var modelData
                visible: modelData.section === "area" ? map.areaOn : map.sideOn
                map: mapItem
                pad: padItem
                section: modelData.section
                edge: modelData.edge
                gripPosition: modelData.grip
            }
        }

        // ----- animated finger preview -----
        FingerPreview {
            z: 45
            anchors.fill: parent
            map: mapItem
            trigger: map.live ? "" : map.previewTrigger
            frozen: map.freezePreview
            visible: !map.live && map.previewTrigger !== ""
        }

        // ----- live contacts -----
        Repeater {
            model: map.live ? DaemonClient.contacts : []
            delegate: Rectangle {
                id: contact
                required property var modelData
                z: 46
                readonly property real size: Math.max(12, Math.min(9 * map.s, 44))
                x: modelData.x_mm * map.s - size / 2
                y: modelData.y_mm * map.s - size / 2
                width: size
                height: size
                radius: size / 2
                color: map.alpha(map.roleColor(modelData.role), 0.85)
                border.width: 2
                border.color: Kirigami.Theme.backgroundColor
                QQC2.Label {
                    anchors.top: parent.bottom
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: contact.modelData.role
                    font: Kirigami.Theme.smallFont
                    visible: map.roomy
                }
            }
        }

        // ----- last gesture toast -----
        Rectangle {
            id: toast
            property string text: ""
            property string iconName: ""
            readonly property real iconSize: Kirigami.Units.iconSizes.small
            z: 60
            anchors.horizontalCenter: parent.horizontalCenter
            y: Math.max(map.areaTop * map.s + Kirigami.Units.smallSpacing, Kirigami.Units.gridUnit * 2)
            width: Math.min(toastLabel.implicitWidth + iconSize + Kirigami.Units.smallSpacing + Kirigami.Units.largeSpacing * 2,
                            pad.width - Kirigami.Units.gridUnit)
            height: toastLabel.implicitHeight + Kirigami.Units.smallSpacing * 2
            radius: height / 2
            color: Kirigami.Theme.highlightColor
            opacity: 0
            visible: opacity > 0

            Kirigami.Icon {
                id: toastIcon
                x: Kirigami.Units.largeSpacing
                anchors.verticalCenter: parent.verticalCenter
                width: toast.iconSize
                height: toast.iconSize
                source: toast.iconName
            }
            QQC2.Label {
                id: toastLabel
                anchors.left: toastIcon.right
                anchors.leftMargin: Kirigami.Units.smallSpacing
                anchors.right: parent.right
                anchors.rightMargin: Kirigami.Units.largeSpacing
                anchors.verticalCenter: parent.verticalCenter
                text: toast.text
                color: Kirigami.Theme.highlightedTextColor
                elide: Text.ElideRight
            }

            SequentialAnimation {
                id: toastAnimation
                NumberAnimation { target: toast; property: "opacity"; to: 0.95; duration: 120 }
                PauseAnimation { duration: 1800 }
                NumberAnimation { target: toast; property: "opacity"; to: 0; duration: 500 }
            }

            Connections {
                target: DaemonClient
                enabled: map.live
                function onGestureFired(trigger: string, action: string) {
                    const g = DaemonClient.lastGesture;
                    toast.text = i18nc("@info %1 gesture, %2 action", "%1 → %2", g.triggerTitle, g.actionLabel);
                    toast.iconName = g.actionIcon ?? "";
                    toastAnimation.restart();
                }
            }
        }
    }
}
