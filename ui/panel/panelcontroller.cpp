// SPDX-License-Identifier: GPL-2.0-or-later
#include "panelcontroller.h"

#include "actionicons.h"

#include <KLocalizedString>
#include <KService>
#include <KWindowEffects>

#include <QDBusConnection>
#include <QDBusConnectionInterface>
#include <QDBusMessage>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QDBusServiceWatcher>
#include <QDateTime>
#include <QGuiApplication>
#include <QJSEngine>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QPainterPath>
#include <QQuickWindow>
#include <QRegion>
#include <QScreen>
#include <QThread>

#if HAVE_LAYERSHELLQT
#include <LayerShellQt/Window>
#endif

using namespace Qt::StringLiterals;

namespace
{
const QString kService = u"io.github.kaget"_s;
const QString kPath = u"/io/github/kaget/Daemon"_s;
const QString kInterface = u"io.github.kaget.Daemon1"_s;
constexpr int kSendIntervalMs = 40;
constexpr int kRefreshIntervalMs = 1500;
/// After a local move, values from the daemon for that control are ignored this long.
constexpr qint64 kLocalHoldMs = 800;
/// The window keeps its own transparent margin for the shadow.
constexpr int kMargin = 0;
/// Matches the fade-out in Panel.qml.
constexpr int kUnmapDelayMs = 170;
constexpr qint64 kActivityIntervalMs = 500;

QVariantMap demoValues()
{
    return {
        {u"brightness"_s, QVariantMap{{u"available"_s, true}, {u"value"_s, 100.0}}},
        {u"volume"_s, QVariantMap{{u"available"_s, true}, {u"value"_s, 53.0}, {u"muted"_s, false}}},
        {u"keyboard_backlight"_s, QVariantMap{{u"available"_s, true}, {u"value"_s, 66.7}, {u"level"_s, 2}, {u"max"_s, 3}}},
    };
}
}

PanelController::PanelController(bool demo, QObject *parent)
    : QObject(parent)
    , m_demo(demo)
{
    s_instance = this;
    m_sendTimer.setSingleShot(true);
    m_sendTimer.setInterval(kSendIntervalMs);
    connect(&m_sendTimer, &QTimer::timeout, this, &PanelController::flushPending);
    m_unmapTimer.setSingleShot(true);
    m_unmapTimer.setInterval(kUnmapDelayMs);
    connect(&m_unmapTimer, &QTimer::timeout, this, [this]() {
        if (!m_shown && m_window)
            m_window->hide();
    });
    m_hideTimer.setSingleShot(true);
    connect(&m_hideTimer, &QTimer::timeout, this, &PanelController::userHide);
    // pick up changes made elsewhere (keys, other apps) while the panel is open
    m_refreshTimer.setInterval(kRefreshIntervalMs);
    connect(&m_refreshTimer, &QTimer::timeout, this, [this]() { call(u"RefreshControls"_s); });

    if (m_demo) {
        m_values = demoValues();
        m_controls = {u"brightness"_s, u"volume"_s, u"action:media.play_pause"_s, u"scale"_s};
        m_scale = 1.55;
        m_daemonRunning = true;
        return;
    }
    auto bus = QDBusConnection::sessionBus();
    bus.connect(kService, kPath, kInterface, u"ControlsChanged"_s, this, SLOT(onControlsChanged(QString)));
    bus.connect(kService, kPath, kInterface, u"StateChanged"_s, this, SLOT(onStateChanged()));
    bus.connect(kService, kPath, kInterface, u"PanelScale"_s, this, SLOT(onPanelScale(double)));
    bus.connect(kService, kPath, kInterface, u"PanelFocus"_s, this, SLOT(onPanelFocus(int)));
    bus.connect(kService, kPath, kInterface, u"Gesture"_s, this, SLOT(onGesture(QString, QString)));
    auto *watcher = new QDBusServiceWatcher(kService, bus, QDBusServiceWatcher::WatchForOwnerChange, this);
    connect(watcher, &QDBusServiceWatcher::serviceOwnerChanged, this, [this](const QString &, const QString &, const QString &owner) {
        setDaemonRunning(!owner.isEmpty());
        if (!owner.isEmpty()) {
            loadConfig();
            call(u"RefreshControls"_s);
        }
    });
    setDaemonRunning(bus.interface()->isServiceRegistered(kService));
    loadConfig();
    call(u"GetControls"_s);
}

PanelController *PanelController::create(QQmlEngine *, QJSEngine *)
{
    QJSEngine::setObjectOwnership(s_instance, QJSEngine::CppOwnership);
    return s_instance;
}

void PanelController::setDaemonRunning(bool running)
{
    if (running != m_daemonRunning) {
        m_daemonRunning = running;
        Q_EMIT daemonRunningChanged();
    }
}

void PanelController::call(const QString &method, const QVariantList &args)
{
    if (m_demo)
        return;
    QDBusMessage msg = QDBusMessage::createMethodCall(kService, kPath, kInterface, method);
    msg.setArguments(args);
    auto *w = new QDBusPendingCallWatcher(QDBusConnection::sessionBus().asyncCall(msg, 3000), this);
    connect(w, &QDBusPendingCallWatcher::finished, this, [this, method](QDBusPendingCallWatcher *w) {
        w->deleteLater();
        const QDBusMessage reply = w->reply();
        if (reply.type() == QDBusMessage::ErrorMessage) {
            if (reply.errorName() == u"org.freedesktop.DBus.Error.ServiceUnknown"_s)
                setDaemonRunning(false);
            return;
        }
        setDaemonRunning(true);
        const QString text = reply.arguments().value(0).toString();
        if (method == u"GetControls"_s)
            applyControls(text);
        else if (method == u"ListActions"_s) {
            const QJsonArray list = QJsonDocument::fromJson(text.toUtf8()).array();
            for (const QJsonValue &v : list)
                m_labels.insert(v[u"action"_s].toString(), v[u"label"_s].toString());
            ++m_catalogRevision;
            Q_EMIT catalogChanged();
        }
        else if (method == u"GetState"_s) {
            const QJsonObject device = QJsonDocument::fromJson(text.toUtf8()).object().value(u"device"_s).toObject();
            const double w = device.value(u"width_mm"_s).toDouble();
            const double h = device.value(u"height_mm"_s).toDouble();
            if (w > 10 && h > 10 && (!qFuzzyCompare(w, m_padWidth) || !qFuzzyCompare(h, m_padHeight))) {
                m_padWidth = w;
                m_padHeight = h;
                Q_EMIT padSizeChanged();
            }
        }
        else if (method == u"GetConfig"_s) {
            const QJsonObject panel = QJsonDocument::fromJson(text.toUtf8()).object().value(u"panel"_s).toObject();
            if (panel.isEmpty())
                return;
            const QStringList controls = panel.value(u"controls"_s).toVariant().toStringList();
            if (!controls.isEmpty())
                m_controls = controls;
            m_position = panel.value(u"position"_s).toString(m_position);
            m_autoHideS = panel.value(u"auto_hide_s"_s).toDouble(m_autoHideS);
            // a scale drag in progress wins over the saved value
            if (QDateTime::currentMSecsSinceEpoch() - m_lastScaleMove > kLocalHoldMs)
                setScale(panel.value(u"scale"_s).toDouble(m_scale));
            Q_EMIT configChanged();
            applyPlacement();
            keepAlive();
        }
    });
}

void PanelController::loadConfig()
{
    call(u"GetConfig"_s);
    call(u"GetState"_s);
    if (m_labels.isEmpty())
        call(u"ListActions"_s);
}

void PanelController::onGesture(const QString &trigger, const QString &action)
{
    if (trigger == u"panel"_s)
        Q_EMIT actionRan(action);
}

void PanelController::runAction(const QString &action)
{
    keepAlive();
    if (m_demo) {
        Q_EMIT actionRan(action);
        return;
    }
    call(u"RunAction"_s, {action});
    // a click runs it through RunAction, which reports as "dbus": flash here
    Q_EMIT actionRan(action);
}

QVariantMap PanelController::slotInfo(const QString &slot) const
{
    if (!slot.startsWith(u"action:"_s)) {
        static const QHash<QString, QString> names{
            {u"brightness"_s, i18n("Screen brightness")},
            {u"volume"_s, i18n("Volume")},
            {u"keyboard_backlight"_s, i18n("Keyboard backlight")},
            {u"scale"_s, i18n("Panel size")},
        };
        return {{u"action"_s, false}, {u"kind"_s, slot}, {u"label"_s, names.value(slot, slot)}, {u"icon"_s, QString()}};
    }
    const QString action = slot.mid(7);
    QString label = m_labels.value(action);
    QString icon = iconForAction(action);
    if (action.startsWith(u"app:"_s)) {
        if (const KService::Ptr service = KService::serviceByStorageId(action.mid(4))) {
            label = service->name();
            if (!service->icon().isEmpty())
                icon = service->icon();
        }
    }
    if (label.isEmpty()) {
        // kwin:<name>, global:<component>:<name>, cmd:<text>, key:/shortcut:<keys>
        label = action.section(u':', action.startsWith(u"global:"_s) ? 2 : 1);
        if (label.isEmpty())
            label = action;
    }
    return {{u"action"_s, true}, {u"kind"_s, action}, {u"label"_s, label}, {u"icon"_s, icon}};
}

void PanelController::onStateChanged()
{
    loadConfig();
}

void PanelController::onPanelScale(double scale)
{
    // a finger on the touchpad's size column: resize live, the daemon saves it on lift
    setScale(scale);
}

void PanelController::onPanelFocus(int column)
{
    if (column != m_focusColumn) {
        m_focusColumn = column;
        Q_EMIT focusColumnChanged();
    }
}

void PanelController::onControlsChanged(const QString &json)
{
    applyControls(json);
}

void PanelController::applyControls(const QString &json)
{
    const QVariantMap incoming = QJsonDocument::fromJson(json.toUtf8()).object().toVariantMap();
    if (incoming.isEmpty())
        return;
    const qint64 now = QDateTime::currentMSecsSinceEpoch();
    for (auto it = incoming.cbegin(); it != incoming.cend(); ++it) {
        // keep what the finger is doing instead of jumping back to a stale reading
        if (now - m_lastLocal.value(it.key(), 0) < kLocalHoldMs && m_values.contains(it.key()))
            continue;
        m_values.insert(it.key(), it.value());
    }
    Q_EMIT valuesChanged();
}

void PanelController::setControl(const QString &kind, double percent)
{
    percent = std::clamp(percent, 0.0, 100.0);
    m_lastLocal.insert(kind, QDateTime::currentMSecsSinceEpoch());
    QVariantMap v = m_values.value(kind).toMap();
    v.insert(u"value"_s, percent);
    m_values.insert(kind, v);
    Q_EMIT valuesChanged();
    m_pending.insert(kind, percent);
    if (!m_sendTimer.isActive())
        m_sendTimer.start();
    keepAlive();
}

void PanelController::flushPending()
{
    for (auto it = m_pending.cbegin(); it != m_pending.cend(); ++it)
        call(u"SetControl"_s, {it.key(), it.value()});
    m_pending.clear();
}

void PanelController::setScale(double scale)
{
    scale = std::clamp(std::round(scale * 100.0) / 100.0, 0.5, 2.0);
    if (qFuzzyCompare(scale, m_scale))
        return;
    m_scale = scale;
    m_lastScaleMove = QDateTime::currentMSecsSinceEpoch();
    Q_EMIT scaleChanged();
    keepAlive();
}

void PanelController::commitScale()
{
    call(u"SetPanelScale"_s, {m_scale});
}

void PanelController::setPointerInside(bool inside)
{
    m_pointerInside = inside;
    if (inside)
        m_usedWithPointer = true;
    keepAlive();
}

void PanelController::keepAlive()
{
    if (!m_shown)
        return;
    if (m_daemonRunning && !m_demo) {
        // the daemon ends panel mode after its idle time; tell it the panel is in use
        m_hideTimer.stop();
        const qint64 now = QDateTime::currentMSecsSinceEpoch();
        if (now - m_lastActivitySent >= kActivityIntervalMs) {
            m_lastActivitySent = now;
            call(u"PanelActivity"_s);
        }
        return;
    }
    if (m_autoHideS <= 0 || m_pointerInside) {
        m_hideTimer.stop();
        return;
    }
    m_hideTimer.start(int(m_autoHideS * 1000));
}

void PanelController::setWindow(QQuickWindow *window)
{
    m_window = window;
    if (!m_window)
        return;
#if HAVE_LAYERSHELLQT
    m_layerShell = QGuiApplication::platformName() == u"wayland"_s;
#endif
    if (!m_layerShell)
        m_window->setFlags(m_window->flags() | Qt::FramelessWindowHint | Qt::WindowStaysOnTopHint | Qt::Tool);

    // with the basic render loop this runs on the GUI thread just before the frame is drawn, so the
    // region goes out with the same surface commit as the card it belongs to
    connect(
        m_window,
        &QQuickWindow::beforeSynchronizing,
        this,
        [this]() {
            if (!m_blurDirty)
                return;
            // basic render loop: GUI thread, the region goes out with this frame; a forced threaded
            // loop calls this on the render thread, where window calls are not allowed
            if (QThread::currentThread() == thread())
                updateBlur();
            else
                QMetaObject::invokeMethod(this, &PanelController::updateBlur, Qt::QueuedConnection);
        },
        Qt::DirectConnection);
    connect(m_window, &QWindow::activeChanged, this, [this]() {
        // clicked somewhere else after using the panel with the pointer
        if (m_window->isActive())
            m_wasActive = m_usedWithPointer;
        else if (m_wasActive && m_shown && !m_pointerInside)
            userHide();
    });
    applyPlacement();
}

void PanelController::applyPlacement()
{
    if (!m_window || m_placedPosition == m_position)
        return;
    const bool wasVisible = m_window->isVisible();
    if (wasVisible)
        m_window->hide(); // layer-shell anchors apply when the surface is mapped again
    m_placedPosition = m_position;
#if HAVE_LAYERSHELLQT
    if (m_layerShell) {
        using LW = LayerShellQt::Window;
        LW *lw = LW::get(m_window);
        lw->setLayer(LW::LayerOverlay);
        lw->setScope(u"kaget-panel"_s);
        lw->setKeyboardInteractivity(LW::KeyboardInteractivityOnDemand);
        lw->setExclusiveZone(0); // stay clear of Plasma panels
        lw->setWantsToBeOnActiveScreen(true);
        // the touchpad drives the panel: keep typing in the window that has focus
        lw->setActivateOnShow(false);
        LW::Anchors a = LW::AnchorNone;
        if (m_position.startsWith(u"top"_s))
            a |= LW::AnchorTop;
        if (m_position.startsWith(u"bottom"_s))
            a |= LW::AnchorBottom;
        if (m_position.endsWith(u"_left"_s))
            a |= LW::AnchorLeft;
        if (m_position.endsWith(u"_right"_s))
            a |= LW::AnchorRight;
        lw->setAnchors(a);
        lw->setMargins(QMargins(kMargin, kMargin, kMargin, kMargin));
    }
#endif
    if (wasVisible)
        m_window->show();
}

void PanelController::setCardGeometry(qreal x, qreal y, qreal width, qreal height, qreal radius)
{
    const QRectF card(x, y, width, height);
    if (card == m_card && qFuzzyCompare(radius + 1, m_cardRadius + 1))
        return;
    m_card = card;
    m_cardRadius = radius;
    // x, y, width and height change one by one during an animation: apply them once per frame
    m_blurDirty = true;
    if (m_window)
        m_window->update();
}

void PanelController::checkEffects()
{
    // asked again on every update: the compositor's protocols are only known after the first
    // round-trip, and the blur effect can be switched on and off at any time
    const bool blur = KWindowEffects::isEffectAvailable(KWindowEffects::BlurBehind);
    const bool contrast = KWindowEffects::isEffectAvailable(KWindowEffects::BackgroundContrast);
    if (blur == m_blurAvailable && contrast == m_contrastAvailable && m_effectsChecked)
        return;
    m_effectsChecked = true;
    m_blurAvailable = blur;
    m_contrastAvailable = contrast;
    Q_EMIT blurAvailableChanged();
}

void PanelController::updateBlur()
{
    if (!m_window || !m_window->isVisible() || m_card.isEmpty())
        return;
    m_blurDirty = false;
    checkEffects();
    QPainterPath path;
    path.addRoundedRect(m_card, m_cardRadius, m_cardRadius);
    const QRegion card(path.toFillPolygon().toPolygon());
    // KWin blurs in a texture the size of the region's bounding box, anchored at its top-left: when
    // that box changes while the card resizes, the textures are reallocated and the downsampling grid
    // and the noise pattern move every frame, so the frosted background shimmers. One pixel in two
    // opposite corners of the (fixed-size) window keeps the box constant; those pixels sit in the
    // transparent margin, far from the card.
    QRegion blur = card;
    blur += QRect(0, 0, 1, 1);
    blur += QRect(m_window->width() - 1, m_window->height() - 1, 1, 1);
    if (m_blurAvailable && m_blurEnabled) {
        KWindowEffects::enableBlurBehind(m_window, true, blur);
        // vibrancy where the compositor still offers it (KWin 6.7 has only blur; its own blur
        // settings carry the saturation)
        if (m_contrastAvailable)
            KWindowEffects::enableBackgroundContrast(m_window, true, 0.85, 1.0, 2.2, blur);
        m_blurSent = true;
    } else if (m_blurSent) {
        // switched off once (Panel.qml has already covered the card with a solid tint)
        KWindowEffects::enableBlurBehind(m_window, false);
        if (m_contrastAvailable)
            KWindowEffects::enableBackgroundContrast(m_window, false);
        m_blurSent = false;
    }
    // clicks on the transparent margin go to the windows below
    m_window->setMask(card);
}

void PanelController::setBlurEnabled(bool enabled)
{
    if (enabled == m_blurEnabled)
        return;
    m_blurEnabled = enabled;
    // applied with the next frame, like every region change
    m_blurDirty = true;
    if (m_window)
        m_window->update();
}

void PanelController::show()
{
    if (!m_window)
        return;
    if (!m_layerShell) {
        // no layer-shell: place it ourselves on the primary screen
        const QRect g = m_window->screen() ? m_window->screen()->availableGeometry() : QRect();
        const int w = m_window->width();
        const int h = m_window->height();
        int x = g.x() + (g.width() - w) / 2;
        int y = g.y() + (g.height() - h) / 2;
        if (m_position.startsWith(u"top"_s))
            y = g.y() + kMargin;
        if (m_position.startsWith(u"bottom"_s))
            y = g.bottom() - h - kMargin;
        if (m_position.endsWith(u"_left"_s))
            x = g.x() + kMargin;
        if (m_position.endsWith(u"_right"_s))
            x = g.right() - w - kMargin;
        m_window->setPosition(x, y);
    }
    m_wasActive = false;
    m_usedWithPointer = m_pointerInside;
    m_unmapTimer.stop();
    m_window->show();
    if (!m_layerShell)
        m_window->requestActivate();
    updateBlur();
    // on a cold start the panel shows before the compositor's blur protocol is bound: look again
    for (const int ms : {50, 250, 1000}) {
        QTimer::singleShot(ms, this, [this, ms]() {
            if (!m_shown || m_blurAvailable)
                return;
            m_blurDirty = true;
            updateBlur();
            if (ms == 1000 && !m_blurAvailable)
                qWarning("kaget-panel: the compositor does not blur behind windows; the panel uses a solid tint");
        });
    }
    if (!m_shown) {
        m_shown = true;
        Q_EMIT shownChanged();
    }
    call(u"RefreshControls"_s);
    if (!m_demo)
        m_refreshTimer.start();
    keepAlive();
}

void PanelController::hide()
{
    m_hideTimer.stop();
    m_refreshTimer.stop();
    flushPending();
    if (m_shown) {
        m_shown = false;
        Q_EMIT shownChanged(); // Panel.qml fades out, then the window is unmapped
    }
    if (m_window && m_window->isVisible())
        m_unmapTimer.start();
}

void PanelController::userHide()
{
    const bool wasShown = m_shown;
    hide();
    onPanelFocus(-1);
    if (wasShown && m_daemonRunning)
        call(u"SetPanelOpen"_s, {false});
}

void PanelController::showSelf()
{
    show();
    if (m_daemonRunning)
        call(u"SetPanelOpen"_s, {true});
}

void PanelController::toggle()
{
    if (m_daemonRunning && !m_demo) {
        call(u"RunAction"_s, {u"kaget.panel"_s});
        return;
    }
    if (m_shown)
        userHide();
    else
        showSelf();
}
