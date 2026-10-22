// SPDX-License-Identifier: GPL-2.0-or-later
// Touchpad control panel: a frosted-glass card with one vertical slider per configured control.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Window
import org.kde.kirigami as Kirigami
import io.github.kaget.panel

Window {
    id: win

    readonly property real target: PanelController.scale
    /// Drawing scale; glides after `target`.
    property real s: target
    /// The window is sized for the largest panel and never resized while the panel changes size:
    /// a layer-shell surface that grows is drawn at its old position for a frame or two.
    readonly property real maxScale: 2.0
    /// The card is a miniature of the touchpad: this many pixels per touchpad millimetre.
    readonly property real pxPerMm: 3.0
    readonly property int count: Math.max(1, PanelController.controls.length)
    readonly property real innerWidth: Math.round(PanelController.padWidth * pxPerMm * s)
    readonly property real innerHeight: Math.round(PanelController.padHeight * pxPerMm * s)
    readonly property real gap: Math.round(10 * s)
    readonly property bool dark: Kirigami.ColorUtils.brightnessForColor(Kirigami.Theme.backgroundColor) === Kirigami.ColorUtils.Dark
    readonly property color ink: dark ? "white" : "#1d1d1f"
    readonly property real radius: Math.round(24 * s)
    readonly property real pad: Math.round(16 * s)
    /// Transparent room around the card for its shadow.
    readonly property int shadow: 28
    readonly property string pos: PanelController.position

    // keeps its speed when a new size arrives mid-way, so steps blend into one smooth change
    Behavior on s {
        // velocity-driven: follows a dragged size slider live (a duration-driven animation restarts
        // its 260 ms on every new step and only catches up once the slider stops)
        SmoothedAnimation {
            id: scaleAnimation
            velocity: 3.0 // scale units per second: a 5 % step takes ~17 ms
            duration: -1
            reversingMode: SmoothedAnimation.Eased
            onRunningChanged: {
                if (!running)
                    settle.restart();
            }
        }
    }
    onTargetChanged: settle.restart()

    // KWin's blur shimmers on a card that changes size, and it cannot fade. So the blur is off while
    // the card resizes or fades in, under a solid tint; once the card stays put the blur comes back
    // under that tint and the tint thins out: the blur appears to fade in.
    /// The size is changing, or changed less than `settle.interval` ago.
    readonly property bool resizing: scaleAnimation.running || settle.running
    /// The card has finished fading in.
    property bool appeared: false
    readonly property bool glass: PanelController.blurAvailable && PanelController.shown && appeared && !resizing
    readonly property real solidTint: dark ? 0.86 : 0.9
    readonly property real glassTint: dark ? 0.26 : 0.34

    Timer {
        id: settle
        interval: 220
    }
    Timer {
        id: appear
        interval: 200 // the card's fade-in
        onTriggered: win.appeared = true
    }
    Timer {
        id: blurOff
        interval: 120 // the tint's fade to solid
        onTriggered: {
            if (!win.glass)
                PanelController.setBlurEnabled(false);
        }
    }
    Connections {
        target: PanelController
        function onShownChanged() {
            if (PanelController.shown) {
                appear.restart();
            } else {
                // closing: the card fades out; take the blur away with it at once
                win.appeared = false;
                PanelController.setBlurEnabled(false);
            }
        }
    }
    onGlassChanged: {
        if (glass)
            PanelController.setBlurEnabled(true); // under the still solid tint
        else if (PanelController.shown)
            blurOff.restart(); // once the tint covers the card
    }

    visible: false
    color: "transparent"
    title: i18nc("@title:window", "KAGeT Control Panel")
    // fixed: from the touchpad size at the largest scale; the rest of the window is transparent and
    // lets clicks through (input and blur follow the card)
    width: Math.ceil((PanelController.padWidth * pxPerMm + 2 * 16) * maxScale) + 2 * shadow + 4
    height: Math.ceil((PanelController.padHeight * pxPerMm + 2 * 16) * maxScale) + 2 * shadow + 4

    function info(kind) {
        return PanelController.values[kind] ?? {};
    }
    // y that stays put on screen: measured from the edge the panel is anchored to
    function stableY(item, y) {
        const p = item.mapToItem(null, 0, y).y;
        if (pos.startsWith("bottom"))
            return p - win.height;
        if (pos.startsWith("top"))
            return p;
        return p - win.height / 2;
    }

    Item {
        id: card

        width: win.innerWidth + 2 * win.pad
        height: win.innerHeight + 2 * win.pad
        // the card sits at the window edge the panel is anchored to
        x: win.pos.endsWith("_left") ? win.shadow : win.pos.endsWith("_right") ? win.width - win.shadow - width : Math.round((win.width - width) / 2)
        y: win.pos.startsWith("top") ? win.shadow : win.pos.startsWith("bottom") ? win.height - win.shadow - height : Math.round((win.height - height) / 2)
        focus: true

        // macOS-like appearance: fade in while growing slightly, the reverse when closing
        opacity: PanelController.shown ? 1 : 0
        scale: PanelController.shown ? 1 : 0.92
        transformOrigin: win.pos.startsWith("top") ? Item.Top : win.pos.startsWith("bottom") ? Item.Bottom : Item.Center
        Behavior on opacity {
            NumberAnimation { duration: 160; easing.type: Easing.OutCubic }
        }
        Behavior on scale {
            NumberAnimation { duration: 240; easing.type: Easing.OutBack; easing.overshoot: 1.1 }
        }

        function report() {
            PanelController.setCardGeometry(x, y, width, height, win.radius);
        }
        onXChanged: report()
        onYChanged: report()
        onWidthChanged: report()
        onHeightChanged: report()
        Component.onCompleted: report()

        Keys.onEscapePressed: PanelController.userHide()

        HoverHandler {
            onHoveredChanged: PanelController.setPointerInside(hovered)
        }

        // soft shadow: rings outside the card only, nothing darkens the glass itself
        Repeater {
            model: 12
            Rectangle {
                required property int index
                anchors.fill: parent
                anchors.margins: -(index + 1) * 2
                radius: win.radius + (index + 1) * 2
                color: "transparent"
                border.width: 2
                border.color: Qt.rgba(0, 0, 0, (win.dark ? 0.07 : 0.035) * Math.pow(1 - index / 12, 2))
            }
        }

        // glass: blurred, saturated background (KWin) under a thin tint
        Rectangle {
            anchors.fill: parent
            radius: win.radius
            // thin over the compositor's blur; solid while there is none (resizing, fading in, or a
            // compositor without blur) so the card always covers what is behind
            property real tint: win.glass ? win.glassTint : win.solidTint
            color: win.dark ? Qt.rgba(0.12, 0.12, 0.14, tint) : Qt.rgba(0.98, 0.98, 0.99, tint)
            Behavior on tint {
                NumberAnimation {
                    duration: win.glass ? 320 : 110
                    easing.type: win.glass ? Easing.InOutQuad : Easing.OutCubic
                }
            }
        }
        // light falling on the top of the glass
        Rectangle {
            anchors.fill: parent
            radius: win.radius
            gradient: Gradient {
                GradientStop { position: 0.0; color: Qt.rgba(1, 1, 1, win.dark ? 0.10 : 0.35) }
                GradientStop { position: 0.45; color: Qt.rgba(1, 1, 1, 0) }
            }
        }
        // hairline edges: bright inside, dark outside
        Rectangle {
            anchors.fill: parent
            anchors.margins: -1
            radius: win.radius + 1
            color: "transparent"
            border.width: 1
            border.color: Qt.rgba(0, 0, 0, win.dark ? 0.45 : 0.12)
        }
        Rectangle {
            anchors.fill: parent
            radius: win.radius
            color: "transparent"
            border.width: 1
            border.color: Qt.rgba(1, 1, 1, win.dark ? 0.16 : 0.6)
        }

        QQC2.Label {
            z: 1
            visible: !PanelController.daemonRunning
            anchors.centerIn: parent
            width: parent.width - 2 * win.pad
            wrapMode: Text.WordWrap
            horizontalAlignment: Text.AlignHCenter
            color: win.ink
            font.pixelSize: Math.round(13 * win.s)
            font.weight: Font.DemiBold
            text: i18n("The KAGeT daemon is not running.")
        }

        // one column per slot, like the touchpad in panel mode
        Row {
            id: row
            x: win.pad
            y: win.pad
            spacing: win.gap

            Repeater {
                model: PanelController.controls

                delegate: Item {
                    id: slot
                    required property string modelData
                    required property int index
                    // depends on catalogRevision so labels update when the action list arrives
                    readonly property var meta: PanelController.catalogRevision >= 0 ? PanelController.slotInfo(modelData) : ({})
                    readonly property bool highlighted: index === PanelController.focusColumn

                    width: Math.floor((win.innerWidth - win.gap * (win.count - 1)) / win.count)
                    height: win.innerHeight

                    Loader {
                        anchors.fill: parent
                        sourceComponent: slot.meta.action ? buttonSlot : sliderSlot
                    }

                    Component {
                        id: buttonSlot
                        PanelButton {
                            id: button
                            s: win.s
                            dark: win.dark
                            ink: win.ink
                            highlighted: slot.highlighted
                            text: slot.meta.label ?? ""
                            iconName: slot.meta.icon ?? ""
                            colorful: (slot.meta.kind ?? "").startsWith("app:")
                            onClicked: PanelController.runAction(slot.meta.kind)
                            Connections {
                                target: PanelController
                                function onActionRan(action) {
                                    if (action === slot.meta.kind)
                                        button.flash();
                                }
                            }
                        }
                    }

                    Component {
                        id: sliderSlot
                        PanelSlider {
                            readonly property string kind: slot.modelData
                            readonly property var info: win.info(kind)
                            readonly property bool isScale: kind === "scale"
                            readonly property int levels: info.max ?? 0

                            s: win.s
                            dark: win.dark
                            ink: win.ink
                            toStable: win.stableY
                            highlighted: slot.highlighted
                            from: isScale ? 50 : (kind === "brightness" ? 1 : 0)
                            to: isScale ? 200 : 100
                            stepSize: kind === "keyboard_backlight" && levels > 0 ? 100 / levels : (isScale ? 5 : 0)
                            value: isScale ? win.target * 100 : (info.value ?? 0)
                            available: isScale || (PanelController.daemonRunning && (info.available ?? false))
                            text: kind === "keyboard_backlight" && levels > 0
                                ? i18nc("@label keyboard backlight level", "%1/%2", Math.round(shown / 100 * levels), levels)
                                : Math.round(shown) + "%"
                            iconName: {
                                switch (kind) {
                                case "brightness":
                                    return "brightness-high";
                                case "volume":
                                    return (info.muted ?? false) || shown === 0 ? "audio-volume-muted" : "audio-volume-high";
                                case "keyboard_backlight":
                                    return "input-keyboard-brightness";
                                default:
                                    return "transform-scale";
                                }
                            }
                            accessibleName: slot.meta.label ?? kind

                            onMoved: v => {
                                if (isScale)
                                    PanelController.scale = v / 100;
                                else
                                    PanelController.setControl(kind, v);
                            }
                            onReleased: v => {
                                if (isScale)
                                    PanelController.commitScale();
                            }
                        }
                    }
                }
            }
        }
    }
}
