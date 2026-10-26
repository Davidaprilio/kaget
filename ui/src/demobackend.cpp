// SPDX-License-Identifier: GPL-2.0-or-later
#include "demobackend.h"

#include <QFile>
#include <QJsonDocument>
#include <QSet>

#include <algorithm>
#include <cmath>
#include <numbers>

using namespace Qt::StringLiterals;

namespace
{
constexpr qint64 kCycleMs = 14000;
constexpr int kFrameMs = 16; // ~60 Hz

QJsonDocument loadDemoFile(const QString &name)
{
    QFile f(u":/demo/"_s + name);
    if (!f.open(QIODevice::ReadOnly)) {
        qWarning("kaget demo: cannot open bundled %s", qPrintable(name));
        return {};
    }
    QJsonParseError error;
    QJsonDocument doc = QJsonDocument::fromJson(f.readAll(), &error);
    if (error.error != QJsonParseError::NoError)
        qWarning("kaget demo: %s: %s", qPrintable(name), qPrintable(error.errorString()));
    return doc;
}

double clamp01(double v)
{
    return std::clamp(v, 0.0, 1.0);
}

// A linear one-finger or multi-finger stroke used by the live simulation.
struct Stroke {
    qint64 start;
    qint64 duration;
    double fromX, fromY, toX, toY;

    double progress(qint64 t) const
    {
        return clamp01(double(t - start) / double(duration));
    }
    double length() const
    {
        return std::hypot(toX - fromX, toY - fromY);
    }
    qint64 timeAtTravel(double mm) const
    {
        return start + qint64(double(duration) * mm / length());
    }
    bool active(qint64 t) const
    {
        return t >= start && t < start + duration;
    }
};
} // namespace

DemoBackend::DemoBackend(QObject *parent)
    : QObject(parent)
{
    m_state = loadDemoFile(u"state.json"_s).object();
    m_config = loadDemoFile(u"config.json"_s).object();
    m_defaults = loadDemoFile(u"defaults.json"_s).object();
    m_triggers = loadDemoFile(u"triggers.json"_s).array();
    m_actions = loadDemoFile(u"actions.json"_s).array();
    m_systemGestures = loadDemoFile(u"system_gestures.json"_s).array();

    m_timer.setInterval(kFrameMs);
    m_timer.setTimerType(Qt::PreciseTimer);
    connect(&m_timer, &QTimer::timeout, this, &DemoBackend::tick);
}

QString DemoBackend::stateJson() const
{
    return QString::fromUtf8(QJsonDocument(m_state).toJson(QJsonDocument::Compact));
}

QVariant DemoBackend::call(const QString &method, const QVariantList &args)
{
    auto json = [](const auto &value) {
        return QString::fromUtf8(QJsonDocument(value).toJson(QJsonDocument::Compact));
    };

    if (method == u"GetState"_s)
        return stateJson();
    if (method == u"GetConfig"_s)
        return json(m_config);
    if (method == u"GetDefaults"_s)
        return json(m_defaults);
    if (method == u"ListTriggers"_s)
        return json(m_triggers);
    if (method == u"ListActions"_s)
        return json(m_actions);
    if (method == u"SystemGestures"_s)
        return json(m_systemGestures);

    if (method == u"SetConfig"_s) {
        QJsonParseError pe;
        const QJsonDocument doc = QJsonDocument::fromJson(args.value(0).toString().toUtf8(), &pe);
        if (pe.error != QJsonParseError::NoError || !doc.isObject())
            return u"Invalid JSON: %1"_s.arg(pe.errorString());
        const QString error = validate(doc.object());
        if (!error.isEmpty())
            return error;
        m_config = doc.object();
        QJsonObject general = m_config.value(u"general"_s).toObject();
        m_state.insert(u"enabled"_s, general.value(u"enabled"_s).toBool(true));
        QTimer::singleShot(0, this, &DemoBackend::stateChanged);
        return QString();
    }

    if (method == u"RunAction"_s) {
        const QString action = args.value(0).toString();
        if (action == u"kaget.pause"_s) {
            m_state.insert(u"paused"_s, !m_state.value(u"paused"_s).toBool());
            QTimer::singleShot(0, this, &DemoBackend::stateChanged);
        } else if (action == u"numberpad.toggle"_s) {
            QJsonObject np = m_state.value(u"numberpad"_s).toObject();
            np.insert(u"active"_s, !np.value(u"active"_s).toBool());
            m_state.insert(u"numberpad"_s, np);
            QTimer::singleShot(0, this, &DemoBackend::stateChanged);
        }
        return true;
    }

    if (method == u"SetEnabled"_s) {
        m_state.insert(u"enabled"_s, args.value(0).toBool());
        QTimer::singleShot(0, this, &DemoBackend::stateChanged);
        return {};
    }

    if (method == u"SetLive"_s) {
        if (args.value(0).toBool()) {
            m_clock.start();
            m_lastT = 0;
            m_timer.start();
        } else {
            m_timer.stop();
            Q_EMIT touches(u"{\"t\":0,\"contacts\":[]}"_s);
        }
        return {};
    }

    if (method == u"LaunchApp"_s)
        return {};

    qWarning("kaget demo: unknown method %s", qPrintable(method));
    return {};
}

QString DemoBackend::validate(const QJsonObject &config) const
{
    const QJsonObject device = m_state.value(u"device"_s).toObject();
    const double w = device.value(u"width_mm"_s).toDouble(100.0);
    const double h = device.value(u"height_mm"_s).toDouble(60.0);

    for (const QString &section : {u"area"_s, u"side_drag"_s}) {
        const QJsonObject s = config.value(section).toObject();
        const double top = s.value(u"top"_s).toDouble();
        const double right = s.value(u"right"_s).toDouble();
        const double bottom = s.value(u"bottom"_s).toDouble();
        const double left = s.value(u"left"_s).toDouble();
        if (top < 0 || right < 0 || bottom < 0 || left < 0)
            return u"%1: distances must not be negative"_s.arg(section);
        if (left + right > w * 0.5 + 1e-6)
            return u"%1: the centre must keep at least 50 % of the width (left + right ≤ %2 mm)"_s.arg(section).arg(w * 0.5, 0, 'f', 1);
        if (top + bottom > h * 0.5 + 1e-6)
            return u"%1: the centre must keep at least 50 % of the height (top + bottom ≤ %2 mm)"_s.arg(section).arg(h * 0.5, 0, 'f', 1);
    }

    const QJsonObject corners = config.value(u"corners"_s).toObject();
    if (corners.value(u"width"_s).toDouble() <= 0 || corners.value(u"height"_s).toDouble() <= 0)
        return u"corners: width and height must be positive"_s;

    QSet<QString> known;
    for (const QJsonValue &t : m_triggers)
        known.insert(t.toObject().value(u"trigger"_s).toString());
    static const QSet<QString> whenRunning{u"new"_s, u"close"_s, u"focus"_s, u"minimize"_s, u"toggle"_s};
    QSet<QString> seen;
    for (const QJsonValue &b : config.value(u"bindings"_s).toArray()) {
        const QJsonObject o = b.toObject();
        const QString trigger = o.value(u"trigger"_s).toString();
        if (!known.contains(trigger))
            return u"bindings: unknown trigger \"%1\""_s.arg(trigger);
        if (seen.contains(trigger))
            return u"bindings: trigger \"%1\" is bound twice"_s.arg(trigger);
        seen.insert(trigger);
        const QString action = o.value(u"action"_s).toString();
        if (action.isEmpty())
            return u"bindings: \"%1\" has no action"_s.arg(trigger);
        if (action == u"key:"_s || action == u"cmd:"_s || action == u"app:"_s || action == u"shortcut:"_s)
            return u"bindings: \"%1\" has an empty action parameter"_s.arg(trigger);
        const QString wr = o.value(u"when_running"_s).toString();
        if (!wr.isEmpty() && !whenRunning.contains(wr))
            return u"bindings: invalid when_running \"%1\""_s.arg(wr);
    }
    return {};
}

QString DemoBackend::bindingFor(const QString &trigger) const
{
    for (const QJsonValue &b : m_config.value(u"bindings"_s).toArray()) {
        const QJsonObject o = b.toObject();
        if (o.value(u"trigger"_s).toString() == trigger)
            return o.value(u"action"_s).toString();
    }
    return {};
}

// ---------------------------------------------------------------------------
// Live simulation

namespace
{
struct Scene {
    double w, h;
    double cw, ch;
    double sdTop, sdLeft;
    double minTravel;
    double step;
    double holdMs;

    // Segments of the 14 s loop.
    Stroke cornerHold() const
    {
        return {2600, 900, w - cw / 2, ch / 2, w - cw / 2, ch / 2};
    }
    Stroke sideTop() const
    {
        return {4100, 1000, 0, sdTop / 2, 0, sdTop / 2 + 30};
    }
    Stroke sliderRight() const
    {
        return {5700, 1400, w - cw / 2, ch / 2, w - cw / 2, h - 10};
    }
    Stroke sideLeft() const
    {
        return {7700, 1100, sdLeft / 2, 0, sdLeft / 2 + 32, 0};
    }
    Stroke swipeCentre() const
    {
        return {9400, 1200, w / 2 + 22, h * 0.62, w / 2 - 22, h * 0.62};
    }
    Stroke sliderTop() const
    {
        return {11200, 1400, cw / 2, ch / 2, cw / 2 + 64, ch / 2};
    }
};

Scene sceneFor(const QJsonObject &state, const QJsonObject &config)
{
    const QJsonObject device = state.value(u"device"_s).toObject();
    const QJsonObject corners = config.value(u"corners"_s).toObject();
    const QJsonObject side = config.value(u"side_drag"_s).toObject();
    Scene s;
    s.w = device.value(u"width_mm"_s).toDouble(100.0);
    s.h = device.value(u"height_mm"_s).toDouble(60.0);
    s.cw = corners.value(u"width"_s).toDouble(8.0);
    s.ch = corners.value(u"height"_s).toDouble(10.0);
    s.holdMs = corners.value(u"hold_ms"_s).toDouble(320);
    s.sdTop = side.value(u"top"_s).toDouble(4.0);
    s.sdLeft = side.value(u"left"_s).toDouble(4.0);
    s.minTravel = side.value(u"min_travel_mm"_s).toDouble(10.0);
    s.step = std::max(1.0, config.value(u"slider"_s).toObject().value(u"step_mm"_s).toDouble(8.0));
    return s;
}
} // namespace

QList<DemoBackend::Contact> DemoBackend::contactsAt(qint64 t) const
{
    const Scene s = sceneFor(m_state, m_config);
    QList<Contact> out;

    // A: one pointer finger wandering through the centre.
    if (t < 2200) {
        const double a = double(t) / 2200.0 * 2.0 * std::numbers::pi;
        out.append({0, s.w / 2 + 20 * std::sin(a), s.h * 0.62 + 9 * std::sin(2 * a), u"pointer"_s});
        return out;
    }

    // B: hold in the top-right corner.
    if (const Stroke st = s.cornerHold(); st.active(t)) {
        out.append({0, st.fromX, st.fromY, u"corner"_s});
        return out;
    }

    // C: three fingers dragged in from the top side-drag band.
    if (const Stroke st = s.sideTop(); st.active(t)) {
        const double p = st.progress(t);
        const double travel = p * st.length();
        for (int i = 0; i < 3; ++i) {
            const double x = s.w / 2 + (i - 1) * 13.0;
            const double y = st.fromY + (st.toY - st.fromY) * p + (i == 1 ? 1.5 : 0.0);
            out.append({i, x, y, travel < s.minTravel ? u"withheld"_s : u"sidedrag"_s});
        }
        return out;
    }

    // D: slider from the top-right corner down the right band.
    if (const Stroke st = s.sliderRight(); st.active(t)) {
        const double p = st.progress(t);
        const double travel = p * st.length();
        out.append({0, st.fromX, st.fromY + (st.toY - st.fromY) * p, travel < 7.0 ? u"corner"_s : u"slider"_s});
        return out;
    }

    // E: four fingers dragged in from the left side-drag band.
    if (const Stroke st = s.sideLeft(); st.active(t)) {
        const double p = st.progress(t);
        const double travel = p * st.length();
        for (int i = 0; i < 4; ++i) {
            const double x = st.fromX + (st.toX - st.fromX) * p + (i == 1 || i == 2 ? 1.5 : 0.0);
            const double y = s.h / 2 + (i - 1.5) * 12.0;
            out.append({i, x, y, travel < s.minTravel ? u"withheld"_s : u"sidedrag"_s});
        }
        return out;
    }

    // F: a three-finger swipe in the centre, passed through to KWin.
    if (const Stroke st = s.swipeCentre(); st.active(t)) {
        const double p = st.progress(t);
        for (int i = 0; i < 3; ++i) {
            const double x = st.fromX + (st.toX - st.fromX) * p + (i == 1 ? -3.0 : 0.0);
            const double y = st.fromY + (i - 1) * 10.0;
            out.append({i, x, y, u"pointer"_s});
        }
        return out;
    }

    // G: horizontal slider from the top-left corner along the top band.
    if (const Stroke st = s.sliderTop(); st.active(t)) {
        const double p = st.progress(t);
        const double travel = p * st.length();
        out.append({0, st.fromX + (st.toX - st.fromX) * p, st.fromY, travel < 7.0 ? u"corner"_s : u"slider"_s});
        return out;
    }

    return out;
}

void DemoBackend::fireGesturesBetween(qint64 from, qint64 to)
{
    const Scene s = sceneFor(m_state, m_config);
    QList<std::pair<qint64, QString>> events;

    events.append({s.cornerHold().start + qint64(s.holdMs), u"corner.top_right.hold"_s});
    events.append({s.sideTop().timeAtTravel(s.minTravel), u"sidedrag.top.3"_s});
    events.append({s.sideLeft().timeAtTravel(s.minTravel), u"sidedrag.left.4"_s});
    const Stroke right = s.sliderRight();
    for (double d = 7.0; d <= right.length(); d += s.step)
        events.append({right.timeAtTravel(d), u"corner.top_right.swipe.down"_s});
    const Stroke top = s.sliderTop();
    for (double d = 7.0; d <= top.length(); d += s.step)
        events.append({top.timeAtTravel(d), u"corner.top_left.swipe.right"_s});

    for (const auto &[time, trigger] : std::as_const(events)) {
        if (time > from && time <= to) {
            const QString action = bindingFor(trigger);
            if (!action.isEmpty())
                Q_EMIT gesture(trigger, action);
        }
    }

    // what the daemon reports when each touch ends
    const auto report = [&](qint64 time, const QString &trigger, int fingers, int steps) {
        if (time <= from || time > to)
            return;
        const QString action = bindingFor(trigger);
        const QStringList p = trigger.split(u'.');
        const bool corner = p.value(0) == u"corner"_s;
        QJsonObject d{
            {u"kind"_s, corner ? u"corner"_s : u"multi"_s},
            {u"fingers"_s, fingers},
            {u"corner"_s, corner ? p.value(1) : QString()},
            {u"trigger"_s, trigger},
            {u"result"_s, action.isEmpty() ? u"kde"_s : steps > 0 ? u"slider"_s : u"fired"_s},
            {u"action"_s, action},
            {u"reason"_s, action.isEmpty() ? u"unbound"_s : QString()},
            {u"span_ms"_s, fingers > 1 ? 30 : 0},
            {u"travel_mm"_s, steps > 0 ? 7.0 + (steps - 1) * s.step : 0.0},
            {u"duration_ms"_s, 600},
            {u"steps"_s, steps},
        };
        Q_EMIT diagnosis(QString::fromUtf8(QJsonDocument(d).toJson(QJsonDocument::Compact)));
    };
    report(s.cornerHold().start + qint64(s.holdMs) + 200, u"corner.top_right.hold"_s, 1, 0);
    report(s.sideTop().start + s.sideTop().duration, u"sidedrag.top.3"_s, 3, 0);
    report(s.sideLeft().start + s.sideLeft().duration, u"sidedrag.left.4"_s, 4, 0);
    const auto strokeSteps = [&](const Stroke &st) { return st.length() < 7.0 ? 0 : 1 + int((st.length() - 7.0) / s.step); };
    report(right.start + right.duration, u"corner.top_right.swipe.down"_s, 1, strokeSteps(right));
    report(top.start + top.duration, u"corner.top_left.swipe.right"_s, 1, strokeSteps(top));
}

void DemoBackend::tick()
{
    const qint64 elapsed = m_clock.elapsed();
    const qint64 t = elapsed % kCycleMs;
    if (t < m_lastT) {
        fireGesturesBetween(m_lastT, kCycleMs);
        fireGesturesBetween(-1, t);
    } else {
        fireGesturesBetween(m_lastT, t);
    }
    m_lastT = t;

    QJsonArray contacts;
    const QList<Contact> list = contactsAt(t);
    for (const Contact &c : list) {
        contacts.append(QJsonObject{
            {u"slot"_s, c.slot},
            {u"x_mm"_s, std::round(c.x * 10.0) / 10.0},
            {u"y_mm"_s, std::round(c.y * 10.0) / 10.0},
            {u"role"_s, c.role},
        });
    }
    const QJsonObject frame{{u"t"_s, elapsed}, {u"contacts"_s, contacts}};
    Q_EMIT touches(QString::fromUtf8(QJsonDocument(frame).toJson(QJsonDocument::Compact)));
}
