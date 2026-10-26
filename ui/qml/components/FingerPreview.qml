// SPDX-License-Identifier: GPL-2.0-or-later
import QtQuick
import QtQuick.Shapes
import org.kde.kirigami as Kirigami

// Animated finger preview for one trigger, drawn on top of the touchpad map:
// dots sliding in from the edge (side drags), moving dots (swipes), converging
// or diverging dots (pinch), pulses (taps) and a filling ring (holds). All geometry is in millimetres, scaled by map.s.
Item {
    id: fp

    required property Item map
    property string trigger: ""
    property bool frozen: false
    property real t: 0

    readonly property real s: map.s
    readonly property color color: map.fingerColor
    readonly property var spec: parse(trigger)
    readonly property var frame: compute(spec, t, map.cfg, map.widthMm, map.heightMm)
    readonly property var trails: computeTrails(spec, map.cfg, map.widthMm, map.heightMm, s)

    onSpecChanged: restart()
    onFrozenChanged: restart()
    onVisibleChanged: restart()

    function restart() {
        anim.stop();
        if (frozen || !visible || spec.kind === "") {
            t = frozenT(spec);
            return;
        }
        t = 0;
        anim.start();
    }

    SequentialAnimation {
        id: anim
        loops: Animation.Infinite
        NumberAnimation {
            target: fp
            property: "t"
            from: 0
            to: 1
            duration: fp.spec.duration
        }
        PauseAnimation {
            duration: 450
        }
    }

    // ----- model -----
    function clamp01(u) {
        return Math.max(0, Math.min(1, u));
    }
    function ease(u) {
        u = clamp01(u);
        return u * u * (3 - 2 * u);
    }
    function seg(t, a, b) {
        return clamp01((t - a) / (b - a));
    }
    function fadeIO(t, a, b) {
        if (t < a)
            return clamp01(t / Math.max(0.001, a));
        if (t > b)
            return clamp01((1 - t) / Math.max(0.001, 1 - b));
        return 1;
    }
    function window01(t, a, b) {
        // 1 inside [a, b] with short fades at both ends.
        const f = 0.03;
        if (t < a - f || t > b + f)
            return 0;
        if (t < a)
            return (t - (a - f)) / f;
        if (t > b)
            return ((b + f) - t) / f;
        return 1;
    }

    function parse(tr) {
        const p = (tr ?? "").split(".");
        const r = {kind: "", n: 0, dir: "", corner: "", edge: "", duration: 1600};
        if (p[0] === "corner" && p.length >= 3) {
            r.corner = p[1];
            r.n = 1;
            if (p[2] === "hold") {
                r.kind = "corner_hold";
                r.duration = 1500;
            } else if (p[2] === "taphold") {
                r.kind = "corner_taphold";
                r.duration = 1700;
            } else {
                r.kind = "corner_swipe";
                r.dir = p[3] ?? "";
                r.duration = 1300;
            }
        } else if (p[0] === "sidedrag" && p.length >= 3) {
            r.kind = "sidedrag";
            r.edge = p[1];
            r.n = parseInt(p[2]);
            r.duration = 1400;
        } else if (["swipe", "pinch"].includes(p[0]) && p.length >= 3) {
            r.kind = p[0];
            r.n = parseInt(p[1]);
            r.dir = p[2];
            r.duration = 1500;
        } else if (["hold", "tap", "doubletap", "taphold"].includes(p[0]) && p.length >= 2) {
            r.kind = p[0];
            r.n = parseInt(p[1]);
            r.duration = p[0] === "tap" ? 1100 : 1700;
        }
        r.n = Math.max(0, Math.min(5, r.n || 0));
        return r;
    }

    function frozenT(spec) {
        switch (spec.kind) {
        case "tap": return 0.24;
        case "doubletap": return 0.45;
        case "corner_hold":
        case "hold": return 0.5;
        case "taphold": return 0.62;
        default: return 0.62;
        }
    }

    function centreOf(cfg, W, H) {
        const l = cfg.area?.left ?? 0, r = cfg.area?.right ?? 0, t = cfg.area?.top ?? 0, b = cfg.area?.bottom ?? 0;
        return {x: l + (W - l - r) / 2, y: t + (H - t - b) / 2};
    }

    function dirVec(dir, corner) {
        switch (dir) {
        case "left": return {x: -1, y: 0};
        case "right": return {x: 1, y: 0};
        case "up": return {x: 0, y: -1};
        case "down": return {x: 0, y: 1};
        case "diagonal": {
            const x = corner.endsWith("left") ? 1 : -1;
            const y = corner.startsWith("top") ? 1 : -1;
            return {x: x * Math.SQRT1_2, y: y * Math.SQRT1_2};
        }
        }
        return {x: 0, y: 0};
    }

    function cluster(n, cx, cy, ux, uy, spacing) {
        const out = [];
        const half = (n - 1) / 2;
        for (let i = 0; i < n; ++i) {
            const k = i - half;
            const ahead = half > 0 ? (1 - Math.abs(k) / half) * 3 : 0;
            out.push({x: cx - uy * k * spacing + ux * ahead, y: cy + ux * k * spacing + uy * ahead});
        }
        return out;
    }

    // Finger positions for motion progress u (0..1).
    function pathAt(spec, u, cfg, W, H) {
        const cw = cfg.corners?.width ?? 8, ch = cfg.corners?.height ?? 10;
        switch (spec.kind) {
        case "corner_hold":
        case "corner_taphold": {
            return [{x: spec.corner.endsWith("left") ? cw / 2 : W - cw / 2, y: spec.corner.startsWith("top") ? ch / 2 : H - ch / 2}];
        }
        case "corner_swipe": {
            const v = dirVec(spec.dir, spec.corner);
            let sx = spec.corner.endsWith("left") ? cw / 2 : W - cw / 2;
            let sy = spec.corner.startsWith("top") ? ch / 2 : H - ch / 2;
            let ex = Math.max(3, Math.min(W - 3, sx + v.x * 20));
            let ey = Math.max(3, Math.min(H - 3, sy + v.y * 20));
            if (Math.hypot(ex - sx, ey - sy) < 9) {
                // Moving outwards: start further inside so the stroke is visible.
                sx = ex - v.x * 12;
                sy = ey - v.y * 12;
            }
            return [{x: sx + (ex - sx) * u, y: sy + (ey - sy) * u}];
        }
        case "sidedrag": {
            const sd = cfg.side_drag ?? {};
            const depth = sd[spec.edge] ?? 4;
            const horizontal = spec.edge === "top" || spec.edge === "bottom";
            const L = horizontal ? W : H;
            const travel = Math.max((sd.min_travel_mm ?? 10) + 8, 20);
            const out = [];
            for (let i = 0; i < spec.n; ++i) {
                const a = L / 2 + (i - (spec.n - 1) / 2) * 12 + ((i % 2) ? 1.5 : 0);
                const d = depth / 2 + travel * u;
                switch (spec.edge) {
                case "top": out.push({x: a, y: d}); break;
                case "bottom": out.push({x: a, y: H - d}); break;
                case "left": out.push({x: d, y: a}); break;
                default: out.push({x: W - d, y: a}); break;
                }
            }
            return out;
        }
        case "swipe": {
            const c = centreOf(cfg, W, H);
            const v = dirVec(spec.dir, "");
            const cx = c.x + v.x * (u - 0.5) * 26;
            const cy = c.y + v.y * (u - 0.5) * 26;
            return cluster(spec.n, cx, cy, v.x, v.y, 11);
        }
        case "pinch": {
            const c = centreOf(cfg, W, H);
            const r = spec.dir === "in" ? 19 - 12 * u : 7 + 12 * u;
            const out = [];
            for (let i = 0; i < spec.n; ++i) {
                const a = (-90 + 20 + i * 360 / spec.n) * Math.PI / 180;
                out.push({x: c.x + r * Math.cos(a), y: c.y + r * Math.sin(a)});
            }
            return out;
        }
        case "hold":
        case "tap":
        case "doubletap":
        case "taphold": {
            const c = centreOf(cfg, W, H);
            return cluster(spec.n, c.x, c.y, 0, -1, 11);
        }
        }
        return [];
    }

    function compute(spec, t, cfg, W, H) {
        const result = {dots: [], ring: null};
        if (spec.kind === "")
            return result;
        const moving = ["corner_swipe", "sidedrag", "swipe", "pinch"].includes(spec.kind);
        const u = moving ? ease(seg(t, 0.15, 0.82)) : 0;
        const pts = pathAt(spec, u, cfg, W, H);

        let o = fadeIO(t, 0.06, 0.92);
        let rip = -1;
        let ring = null;

        switch (spec.kind) {
        case "corner_hold":
            ring = {x: pts[0].x, y: pts[0].y, r: 6.5, p: ease(seg(t, 0.12, 0.72))};
            break;
        case "corner_taphold":
            o = Math.max(window01(t, 0.06, 0.18), window01(t, 0.32, 0.88));
            if (t < 0.3)
                rip = seg(t, 0.06, 0.3);
            if (t >= 0.32)
                ring = {x: pts[0].x, y: pts[0].y, r: 6.5, p: ease(seg(t, 0.36, 0.84))};
            break;
        case "tap":
            o = window01(t, 0.14, 0.3);
            rip = seg(t, 0.14, 0.62);
            break;
        case "doubletap":
            o = Math.max(window01(t, 0.1, 0.24), window01(t, 0.42, 0.56));
            rip = t < 0.4 ? seg(t, 0.1, 0.38) : seg(t, 0.42, 0.72);
            break;
        case "hold": {
            o = window01(t, 0.08, 0.86);
            const c = centroid(pts);
            ring = {x: c.x, y: c.y, r: (spec.n - 1) * 5.5 + 8, p: ease(seg(t, 0.14, 0.74))};
            break;
        }
        case "taphold": {
            o = Math.max(window01(t, 0.06, 0.18), window01(t, 0.32, 0.88));
            if (t < 0.3)
                rip = seg(t, 0.06, 0.3);
            const c = centroid(pts);
            if (t >= 0.32)
                ring = {x: c.x, y: c.y, r: (spec.n - 1) * 5.5 + 8, p: ease(seg(t, 0.36, 0.84))};
            break;
        }
        }

        for (let i = 0; i < pts.length; ++i)
            result.dots.push({x: pts[i].x, y: pts[i].y, o: o, rip: rip});
        result.ring = ring;
        return result;
    }

    function centroid(pts) {
        let x = 0, y = 0;
        for (const p of pts) {
            x += p.x;
            y += p.y;
        }
        return {x: x / Math.max(1, pts.length), y: y / Math.max(1, pts.length)};
    }

    function computeTrails(spec, cfg, W, H, s) {
        const moving = ["corner_swipe", "sidedrag", "swipe", "pinch"].includes(spec.kind);
        if (!moving)
            return [];
        const steps = 18;
        const trails = [];
        for (let k = 0; k <= steps; ++k) {
            const pts = pathAt(spec, k / steps, cfg, W, H);
            for (let i = 0; i < pts.length; ++i) {
                if (!trails[i])
                    trails[i] = [];
                trails[i].push(Qt.point(pts[i].x * s, pts[i].y * s));
            }
        }
        // Arrow head at the end of each trail.
        return trails.map(points => {
            const a = points[points.length - 2], b = points[points.length - 1];
            const len = Math.max(0.001, Math.hypot(b.x - a.x, b.y - a.y));
            const ux = (b.x - a.x) / len, uy = (b.y - a.y) / len;
            const h = Math.max(5, 2.2 * s);
            const head = [
                Qt.point(b.x - ux * h - uy * h * 0.6, b.y - uy * h + ux * h * 0.6),
                Qt.point(b.x, b.y),
                Qt.point(b.x - ux * h + uy * h * 0.6, b.y - uy * h - ux * h * 0.6)
            ];
            return {points: points, head: head};
        });
    }

    // ----- rendering -----
    Repeater {
        model: fp.trails.length
        delegate: Shape {
            id: trail
            required property int index
            readonly property var trailData: fp.trails[index] ?? {points: [], head: []}
            anchors.fill: parent
            opacity: 0.55
            ShapePath {
                strokeColor: fp.color
                strokeWidth: 2
                fillColor: "transparent"
                strokeStyle: ShapePath.DashLine
                dashPattern: [3, 3]
                capStyle: ShapePath.RoundCap
                PathPolyline {
                    path: trail.trailData.points
                }
            }
            ShapePath {
                strokeColor: fp.color
                strokeWidth: 2
                fillColor: "transparent"
                capStyle: ShapePath.RoundCap
                joinStyle: ShapePath.RoundJoin
                PathPolyline {
                    path: trail.trailData.head
                }
            }
        }
    }

    // Hold ring: faint track plus the filling arc.
    Shape {
        id: ring
        readonly property var r: fp.frame.ring
        anchors.fill: parent
        visible: r !== null && r !== undefined
        ShapePath {
            strokeColor: Qt.rgba(fp.color.r, fp.color.g, fp.color.b, 0.25)
            strokeWidth: 3
            fillColor: "transparent"
            PathAngleArc {
                centerX: (ring.r?.x ?? 0) * fp.s
                centerY: (ring.r?.y ?? 0) * fp.s
                radiusX: (ring.r?.r ?? 0) * fp.s
                radiusY: (ring.r?.r ?? 0) * fp.s
                startAngle: -90
                sweepAngle: 360
            }
        }
        ShapePath {
            strokeColor: fp.color
            strokeWidth: 3.5
            fillColor: "transparent"
            capStyle: ShapePath.RoundCap
            PathAngleArc {
                centerX: (ring.r?.x ?? 0) * fp.s
                centerY: (ring.r?.y ?? 0) * fp.s
                radiusX: (ring.r?.r ?? 0) * fp.s
                radiusY: (ring.r?.r ?? 0) * fp.s
                startAngle: -90
                sweepAngle: 360 * Math.max(0.001, ring.r?.p ?? 0)
            }
        }
    }

    Repeater {
        model: 5
        delegate: Item {
            id: dot
            required property int index
            readonly property var d: fp.frame.dots[index]
            readonly property real size: Math.max(12, Math.min(8.5 * fp.s, 42))
            readonly property real rip: d?.rip ?? -1
            visible: d !== undefined && (d.o > 0.01 || (rip >= 0 && rip < 1))
            x: (d?.x ?? 0) * fp.s - size / 2
            y: (d?.y ?? 0) * fp.s - size / 2
            width: size
            height: size

            Rectangle {
                // Tap ripple.
                anchors.centerIn: parent
                readonly property real p: dot.rip
                visible: p > 0 && p < 1
                width: dot.size * (1 + p * 1.6)
                height: width
                radius: width / 2
                color: "transparent"
                border.width: 2
                border.color: fp.color
                opacity: 1 - Math.max(0, p)
            }
            Rectangle {
                anchors.fill: parent
                radius: width / 2
                opacity: dot.d?.o ?? 0
                color: Qt.rgba(fp.color.r, fp.color.g, fp.color.b, 0.85)
                border.width: 2
                border.color: Kirigami.Theme.backgroundColor
            }
        }
    }
}
