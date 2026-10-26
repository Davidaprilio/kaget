// SPDX-License-Identifier: GPL-2.0-or-later
#include "daemonclient.h"
#include "actionicons.h"

#include "demobackend.h"
#include "helpers.h"

#include <KLocalizedString>
#include <KApplicationTrader>
#include <KService>

#include <QDBusConnection>
#include <QDBusConnectionInterface>
#include <QDBusMessage>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QDBusServiceWatcher>
#include <QCoreApplication>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QMimeDatabase>
#include <QStandardPaths>
#include <QUrl>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QProcess>
#include <QTimer>

#include <algorithm>
#include <cmath>

using namespace Qt::StringLiterals;

namespace
{
const QString kService = u"io.github.kaget"_s;
const QString kPath = u"/io/github/kaget/Daemon"_s;
const QString kInterface = u"io.github.kaget.Daemon1"_s;

constexpr int kCallTimeoutMs = 8000;

enum LoadedPart {
    StatePart = 1,
    ConfigPart = 2,
};

bool isSystemOwnedBySpec(const QString &trigger)
{
    // System-owned triggers, v1 list: swipe.3.*, swipe.4.*, pinch.3.*, pinch.4.*, tap.3
    return trigger.startsWith(u"swipe.3."_s) || trigger.startsWith(u"swipe.4."_s) || trigger.startsWith(u"pinch.3."_s)
        || trigger.startsWith(u"pinch.4."_s) || trigger == u"tap.3"_s;
}

QString cornerName(const QString &corner)
{
    if (corner == u"top_left"_s)
        return i18nc("@label touchpad corner", "Top-left corner");
    if (corner == u"top_right"_s)
        return i18nc("@label touchpad corner", "Top-right corner");
    if (corner == u"bottom_left"_s)
        return i18nc("@label touchpad corner", "Bottom-left corner");
    if (corner == u"bottom_right"_s)
        return i18nc("@label touchpad corner", "Bottom-right corner");
    return corner;
}

QString edgeName(const QString &edge)
{
    if (edge == u"top"_s)
        return i18nc("@label touchpad edge", "Top edge");
    if (edge == u"right"_s)
        return i18nc("@label touchpad edge", "Right edge");
    if (edge == u"bottom"_s)
        return i18nc("@label touchpad edge", "Bottom edge");
    if (edge == u"left"_s)
        return i18nc("@label touchpad edge", "Left edge");
    return edge;
}

QString edgeArrow(const QString &edge)
{
    // Arrow pointing inwards from the edge.
    if (edge == u"top"_s)
        return u"↓"_s;
    if (edge == u"bottom"_s)
        return u"↑"_s;
    if (edge == u"left"_s)
        return u"→"_s;
    if (edge == u"right"_s)
        return u"←"_s;
    return {};
}

QString directionArrow(const QString &dir)
{
    if (dir == u"left"_s)
        return u"←"_s;
    if (dir == u"right"_s)
        return u"→"_s;
    if (dir == u"up"_s)
        return u"↑"_s;
    if (dir == u"down"_s)
        return u"↓"_s;
    if (dir == u"diagonal"_s)
        return u"↘"_s;
    if (dir == u"in"_s)
        return u"→←"_s;
    if (dir == u"out"_s)
        return u"←→"_s;
    if (dir == u"cw"_s)
        return u"↻"_s;
    if (dir == u"ccw"_s)
        return u"↺"_s;
    return {};
}

QString cornerSwipeTitle(const QString &dir)
{
    if (dir == u"left"_s)
        return i18nc("@label corner gesture", "Swipe left");
    if (dir == u"right"_s)
        return i18nc("@label corner gesture", "Swipe right");
    if (dir == u"up"_s)
        return i18nc("@label corner gesture", "Swipe up");
    if (dir == u"down"_s)
        return i18nc("@label corner gesture", "Swipe down");
    if (dir == u"diagonal"_s)
        return i18nc("@label corner gesture", "Swipe diagonally");
    return i18nc("@label corner gesture", "Swipe %1", dir);
}

QString multiFingerTitle(const QString &kind, const QString &dir, int n)
{
    if (kind == u"swipe"_s) {
        if (dir == u"left"_s)
            return i18nc("@label gesture", "%1-finger swipe left", n);
        if (dir == u"right"_s)
            return i18nc("@label gesture", "%1-finger swipe right", n);
        if (dir == u"up"_s)
            return i18nc("@label gesture", "%1-finger swipe up", n);
        if (dir == u"down"_s)
            return i18nc("@label gesture", "%1-finger swipe down", n);
    } else if (kind == u"pinch"_s) {
        if (dir == u"in"_s)
            return i18nc("@label gesture", "%1-finger pinch in", n);
        if (dir == u"out"_s)
            return i18nc("@label gesture", "%1-finger pinch out", n);
    } else if (kind == u"hold"_s) {
        return i18nc("@label gesture", "%1-finger hold", n);
    } else if (kind == u"tap"_s) {
        return i18nc("@label gesture", "%1-finger tap", n);
    } else if (kind == u"doubletap"_s) {
        return i18nc("@label gesture", "%1-finger double tap", n);
    } else if (kind == u"taphold"_s) {
        return i18nc("@label gesture", "%1-finger tap and hold", n);
    }
    return i18nc("@label gesture", "%1-finger %2 %3", n, kind, dir);
}

QString groupLabel(const QString &group)
{
    if (group == u"corners"_s)
        return i18nc("@title gesture group", "Corners");
    if (group == u"side_drag"_s)
        return i18nc("@title gesture group", "Side drag");
    if (group == u"swipe"_s)
        return i18nc("@title gesture group", "Swipe");
    if (group == u"pinch"_s)
        return i18nc("@title gesture group", "Pinch");
    if (group == u"taps"_s)
        return i18nc("@title gesture group", "Taps");
    return group.isEmpty() ? i18nc("@title gesture group", "Other") : group;
}

int groupOrder(const QString &group)
{
    static const QStringList order{u"corners"_s, u"side_drag"_s, u"swipe"_s, u"pinch"_s, u"taps"_s};
    const int i = order.indexOf(group);
    return i < 0 ? order.size() : i;
}

QString normalizeGroup(const QString &raw)
{
    QString g = raw.toLower();
    g.remove(u' ');
    g.remove(u'_');
    g.remove(u'-');
    if (g.startsWith(u"corner"_s))
        return u"corners"_s;
    if (g.startsWith(u"sidedrag"_s))
        return u"side_drag"_s;
    if (g.startsWith(u"swipe"_s))
        return u"swipe"_s;
    if (g.startsWith(u"pinch"_s))
        return u"pinch"_s;
    if (g.startsWith(u"tap"_s) || g.startsWith(u"hold"_s))
        return u"taps"_s;
    return raw;
}

// Known slider inverses, used when ListActions() gives no hint.
QString knownInverse(const QString &action)
{
    static const QHash<QString, QString> pairs{
        {u"volume.up"_s, u"volume.down"_s},
        {u"brightness.up"_s, u"brightness.down"_s},
        {u"keyboard_backlight.up"_s, u"keyboard_backlight.down"_s},
        {u"media.next"_s, u"media.previous"_s},
        {u"numberpad.brightness_up"_s, u"numberpad.brightness_down"_s},
        {u"kwin:Switch One Desktop to the Left"_s, u"kwin:Switch One Desktop to the Right"_s},
        {u"kwin:Switch One Desktop Up"_s, u"kwin:Switch One Desktop Down"_s},
    };
    for (auto it = pairs.cbegin(); it != pairs.cend(); ++it) {
        if (it.key() == action)
            return it.value();
        if (it.value() == action)
            return it.key();
    }
    return {};
}


bool isIntegerKey(const QString &key)
{
    return key.endsWith(u"_ms"_s) || key == u"version"_s;
}

bool setPath(QVariantMap &map, QStringList path, const QVariant &value)
{
    if (path.isEmpty())
        return false;
    const QString key = path.takeFirst();
    if (path.isEmpty()) {
        map.insert(key, value);
        return true;
    }
    QVariantMap child = map.value(key).toMap();
    const bool ok = setPath(child, path, value);
    map.insert(key, child);
    return ok;
}

QVariant getPath(const QVariantMap &map, const QStringList &path)
{
    QVariant current = map;
    for (const QString &key : path) {
        const QVariantMap m = current.toMap();
        const auto it = m.constFind(key);
        if (it == m.cend())
            return {};
        current = it.value();
    }
    return current;
}
} // namespace

DaemonClient *DaemonClient::s_instance = nullptr;

DaemonClient::DaemonClient(bool demo, QObject *parent)
    : QObject(parent)
    , m_demo(demo)
{
    s_instance = this;

    if (m_demo) {
        m_demoBackend = new DemoBackend(this);
        connect(m_demoBackend, &DemoBackend::touches, this, &DaemonClient::onTouchesSignal);
        connect(m_demoBackend, &DemoBackend::gesture, this, &DaemonClient::onGestureSignal);
        connect(m_demoBackend, &DemoBackend::diagnosis, this, &DaemonClient::onDiagnosisSignal);
        connect(m_demoBackend, &DemoBackend::stateChanged, this, &DaemonClient::onStateChangedSignal);
        m_probing = false;
        m_connected = true;
        m_serviceOwner = u"(demo)"_s;
        QTimer::singleShot(0, this, &DaemonClient::refresh);
        return;
    }

    QDBusConnection bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        m_probing = false;
        m_lastError = i18n("Cannot connect to the D-Bus session bus.");
        return;
    }

    bus.connect(kService, kPath, kInterface, u"StateChanged"_s, this, SLOT(onStateChangedSignal()));
    bus.connect(kService, kPath, kInterface, u"Touches"_s, this, SLOT(onTouchesSignal(QString)));
    bus.connect(kService, kPath, kInterface, u"Gesture"_s, this, SLOT(onGestureSignal(QString, QString)));
    bus.connect(kService, kPath, kInterface, u"Diagnosis"_s, this, SLOT(onDiagnosisSignal(QString)));

    m_watcher = new QDBusServiceWatcher(kService, bus, QDBusServiceWatcher::WatchForOwnerChange, this);
    connect(m_watcher, &QDBusServiceWatcher::serviceOwnerChanged, this, [this](const QString &, const QString &, const QString &newOwner) {
        setConnected(!newOwner.isEmpty(), newOwner);
    });

    // Initial probe: who owns io.github.kaget right now?
    QDBusMessage probe = QDBusMessage::createMethodCall(u"org.freedesktop.DBus"_s,
                                                        u"/org/freedesktop/DBus"_s,
                                                        u"org.freedesktop.DBus"_s,
                                                        u"GetNameOwner"_s);
    probe.setArguments({kService});
    auto *watcher = new QDBusPendingCallWatcher(bus.asyncCall(probe, kCallTimeoutMs), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *w) {
        w->deleteLater();
        QDBusPendingReply<QString> reply = *w;
        m_probing = false;
        if (reply.isError()) {
            setConnected(false, {});
            Q_EMIT connectedChanged();
        } else {
            setConnected(true, reply.value());
        }
    });
}

DaemonClient::~DaemonClient()
{
    if (!m_demo && m_live && m_connected) {
        QDBusMessage msg = QDBusMessage::createMethodCall(kService, kPath, kInterface, u"SetLive"_s);
        msg.setArguments({false});
        QDBusConnection::sessionBus().call(msg, QDBus::Block, 300);
    }
    if (s_instance == this)
        s_instance = nullptr;
}

DaemonClient *DaemonClient::instance()
{
    return s_instance;
}

DaemonClient *DaemonClient::create(QQmlEngine *, QJSEngine *)
{
    Q_ASSERT(s_instance);
    QJSEngine::setObjectOwnership(s_instance, QJSEngine::CppOwnership);
    return s_instance;
}

QString DaemonClient::serviceName() const
{
    return kService;
}

QString DaemonClient::objectPath() const
{
    return kPath;
}

QString DaemonClient::interfaceName() const
{
    return kInterface;
}

// ---------------------------------------------------------------------------
// Calls

void DaemonClient::callDaemon(const QString &method, const QVariantList &args, ReplyFn onReply, ErrorFn onError)
{
    if (m_demo) {
        const QVariant result = m_demoBackend->call(method, args);
        QTimer::singleShot(0, this, [onReply, result]() {
            if (onReply)
                onReply(result);
        });
        return;
    }

    QDBusMessage msg = QDBusMessage::createMethodCall(kService, kPath, kInterface, method);
    msg.setArguments(args);
    auto *watcher = new QDBusPendingCallWatcher(QDBusConnection::sessionBus().asyncCall(msg, kCallTimeoutMs), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this, method, onReply, onError](QDBusPendingCallWatcher *w) {
        w->deleteLater();
        const QDBusMessage reply = w->reply();
        if (reply.type() == QDBusMessage::ErrorMessage) {
            const QString message = reply.errorMessage().isEmpty() ? reply.errorName() : reply.errorMessage();
            if (onError) {
                onError(message);
            } else if (m_connected) {
                setLastError(i18n("D-Bus call %1 failed: %2", method, message));
            }
            return;
        }
        if (onReply)
            onReply(reply.arguments().value(0));
    });
}

void DaemonClient::setConnected(bool connected, const QString &owner)
{
    m_probing = false;
    if (m_connected == connected && m_serviceOwner == owner)
        return;
    m_connected = connected;
    m_serviceOwner = owner;
    Q_EMIT connectedChanged();

    if (connected) {
        refresh();
        if (m_live)
            callDaemon(u"SetLive"_s, {true}, {});
    } else if (!m_contacts.isEmpty()) {
        m_contacts.clear();
        Q_EMIT contactsChanged();
    }
}

void DaemonClient::refresh()
{
    if (!m_connected)
        return;
    fetchState();
    fetchConfig(false);
    fetchLists();
}

void DaemonClient::markLoaded(int part)
{
    m_loadedParts |= part;
    const bool ready = (m_loadedParts & (StatePart | ConfigPart)) == (StatePart | ConfigPart);
    if (ready != m_ready) {
        m_ready = ready;
        Q_EMIT readyChanged();
    }
}

void DaemonClient::fetchState()
{
    callDaemon(u"GetState"_s, {}, [this](const QVariant &v) {
        QString error;
        const QVariant parsed = parseJson(v.toString(), &error);
        if (!error.isEmpty()) {
            setLastError(i18n("GetState returned invalid JSON: %1", error));
            return;
        }
        m_state = parsed.toMap();
        m_stateJson = QString::fromUtf8(QJsonDocument(QJsonObject::fromVariantMap(m_state)).toJson(QJsonDocument::Indented));
        Q_EMIT stateChanged();
        markLoaded(StatePart);
    });
}

void DaemonClient::fetchConfig(bool adoptIntoDraft)
{
    callDaemon(u"GetConfig"_s, {}, [this, adoptIntoDraft](const QVariant &v) {
        QString error;
        const QVariant parsed = parseJson(v.toString(), &error);
        if (!error.isEmpty()) {
            setLastError(i18n("GetConfig returned invalid JSON: %1", error));
            return;
        }
        const QVariantMap previous = m_config;
        m_config = parsed.toMap();
        Q_EMIT configChanged();

        if (adoptIntoDraft || !m_dirty || !m_ready) {
            setDraft(m_config);
            setExternalChange(false);
        } else if (canonicalJson(previous) != canonicalJson(m_config)) {
            // The user has unsaved edits and the file changed under us.
            setExternalChange(true);
            updateDirty();
        }
        markLoaded(ConfigPart);
    });
}

void DaemonClient::fetchLists()
{
    callDaemon(u"ListTriggers"_s, {}, [this](const QVariant &v) {
        m_triggers = parseJson(v.toString()).toList();
        m_triggerIndex.clear();
        for (const QVariant &t : std::as_const(m_triggers)) {
            const QVariantMap m = t.toMap();
            m_triggerIndex.insert(m.value(u"trigger"_s).toString(), m);
        }
        Q_EMIT triggersChanged();
    });
    callDaemon(u"ListActions"_s, {}, [this](const QVariant &v) {
        m_actions = parseJson(v.toString()).toList();
        m_actionIndex.clear();
        for (const QVariant &a : std::as_const(m_actions)) {
            const QVariantMap m = a.toMap();
            m_actionIndex.insert(m.value(u"action"_s).toString(), m);
        }
        Q_EMIT actionsChanged();
        // Action labels are used by bindings everywhere.
        Q_EMIT draftChanged();
    });
    callDaemon(u"SystemGestures"_s, {}, [this](const QVariant &v) {
        m_systemGestures = parseJson(v.toString()).toList();
        Q_EMIT systemGesturesChanged();
    });
}

void DaemonClient::apply()
{
    if (!m_connected) {
        setLastError(i18n("The KAGeT daemon is not running, the configuration cannot be applied."));
        return;
    }
    if (m_applying)
        return;
    m_applying = true;
    Q_EMIT applyingChanged();

    const QString json = QString::fromUtf8(QJsonDocument(QJsonObject::fromVariantMap(m_draft)).toJson(QJsonDocument::Compact));
    callDaemon(
        u"SetConfig"_s,
        {json},
        [this](const QVariant &v) {
            m_applying = false;
            Q_EMIT applyingChanged();
            const QString error = v.toString();
            if (!error.isEmpty()) {
                setLastError(error);
                Q_EMIT applied(false, error);
                return;
            }
            setLastError({});
            setExternalChange(false);
            // Adopt what the daemon actually stored (it may clamp values).
            m_config = m_draft;
            Q_EMIT configChanged();
            updateDirty();
            fetchConfig(true);
            Q_EMIT applied(true, {});
            Q_EMIT notify(i18n("Configuration applied."), false);
        },
        [this](const QString &message) {
            m_applying = false;
            Q_EMIT applyingChanged();
            const QString error = i18n("Could not apply the configuration: %1", message);
            setLastError(error);
            Q_EMIT applied(false, error);
        });
}

void DaemonClient::revert()
{
    setDraft(m_config);
    setExternalChange(false);
    setLastError({});
}

void DaemonClient::resetToDefaults(bool includeBindings)
{
    if (!m_connected) {
        setLastError(i18n("The KAGeT daemon is not running, defaults are not available."));
        return;
    }
    callDaemon(u"GetDefaults"_s, {}, [this, includeBindings](const QVariant &v) {
        QString error;
        QVariantMap defaults = parseJson(v.toString(), &error).toMap();
        if (!error.isEmpty() || defaults.isEmpty()) {
            setLastError(i18n("GetDefaults returned invalid JSON: %1", error));
            return;
        }
        if (!includeBindings)
            defaults.insert(u"bindings"_s, m_draft.value(u"bindings"_s));
        setDraft(defaults);
        Q_EMIT notify(includeBindings ? i18n("All settings and gestures were reset to the defaults. Press Apply to save.")
                                      : i18n("Settings were reset to the defaults. Press Apply to save."),
                      false);
    });
}

void DaemonClient::runAction(const QString &action)
{
    if (action.isEmpty())
        return;
    if (!m_connected) {
        Q_EMIT notify(i18n("The KAGeT daemon is not running."), true);
        return;
    }
    const QString label = actionInfo(action).value(u"label"_s).toString();
    callDaemon(
        u"RunAction"_s,
        {action},
        [this, action, label](const QVariant &v) {
            const bool ok = v.toBool();
            Q_EMIT actionTested(action, ok);
            if (m_demo)
                Q_EMIT notify(i18n("Demo mode: “%1” would run now.", label), false);
            else
                Q_EMIT notify(ok ? i18n("Ran “%1”.", label) : i18n("The daemon could not run “%1”.", label), !ok);
        },
        [this, action, label](const QString &message) {
            Q_EMIT actionTested(action, false);
            Q_EMIT notify(i18n("Running “%1” failed: %2", label, message), true);
        });
}

void DaemonClient::setDaemonEnabled(bool enabled)
{
    if (!m_connected)
        return;
    callDaemon(u"SetEnabled"_s, {enabled}, [this](const QVariant &) {
        fetchState();
    });
}

void DaemonClient::togglePause()
{
    if (!m_connected)
        return;
    callDaemon(u"RunAction"_s, {u"kaget.pause"_s}, [this](const QVariant &) {
        fetchState();
    });
}

void DaemonClient::setLive(bool live)
{
    if (m_live == live)
        return;
    m_live = live;
    Q_EMIT liveChanged();
    if (!live && !m_contacts.isEmpty()) {
        m_contacts.clear();
        Q_EMIT contactsChanged();
    }
    if (m_connected)
        callDaemon(u"SetLive"_s, {live}, {});
}

namespace
{
constexpr auto kOldService = "asus-numberpad";

int runQuiet(const QString &program, const QStringList &args, QString *out = nullptr)
{
    QProcess p;
    p.start(program, args);
    if (!p.waitForFinished(3000))
        return -1;
    if (out)
        *out = QString::fromLocal8Bit(p.readAllStandardOutput()).trimmed();
    return p.exitStatus() == QProcess::NormalExit ? p.exitCode() : -1;
}

QString userUnitPath()
{
    return QStandardPaths::writableLocation(QStandardPaths::GenericConfigLocation) + u"/systemd/user/kagetd.service"_s;
}

QString findDaemonBinary()
{
    const QString appDir = QCoreApplication::applicationDirPath();
    const QStringList candidates{
        QStandardPaths::findExecutable(u"kagetd"_s),
        QDir::homePath() + u"/.local/bin/kagetd"_s,
        appDir + u"/kagetd"_s,
        // Development tree: ui/build/bin/kaget -> target/release/kagetd
        QDir::cleanPath(appDir + u"/../../../target/release/kagetd"_s),
    };
    for (const QString &c : candidates) {
        const QFileInfo fi(c);
        if (!c.isEmpty() && fi.isFile() && fi.isExecutable())
            return fi.absoluteFilePath();
    }
    return {};
}
} // namespace

QVariantMap DaemonClient::inspectSetup() const
{
    QString listed;
    runQuiet(u"systemctl"_s, {u"--user"_s, u"list-unit-files"_s, u"kagetd.service"_s, u"--no-legend"_s}, &listed);
    const bool unitInstalled = listed.contains(u"kagetd.service"_s);
    const bool conflict = runQuiet(u"systemctl"_s, {u"--user"_s, u"is-active"_s, u"--quiet"_s, QString::fromLatin1(kOldService)}) == 0;
    return {
        {u"unitInstalled"_s, unitInstalled},
        {u"binary"_s, unitInstalled ? QString() : findDaemonBinary()},
        {u"conflict"_s, conflict ? QString::fromLatin1(kOldService) : QString()},
    };
}

void DaemonClient::startDaemon()
{
    if (m_demo) {
        runSystemctl(u"start"_s);
        return;
    }
    m_setup = inspectSetup();
    const bool installed = m_setup.value(u"unitInstalled"_s).toBool();
    if (installed && m_setup.value(u"conflict"_s).toString().isEmpty()) {
        installAndStart(false);
        return;
    }
    Q_EMIT setupNeeded(m_setup);
}

void DaemonClient::installAndStart(bool stopConflict)
{
    if (m_setup.isEmpty())
        m_setup = inspectSetup();
    QList<QStringList> commands;
    if (!m_setup.value(u"unitInstalled"_s).toBool()) {
        const QString binary = m_setup.value(u"binary"_s).toString();
        if (binary.isEmpty()) {
            Q_EMIT notify(i18n("kagetd was not found. Build it (cargo build --release) or install the KAGeT package first."), true);
            return;
        }
        const QString unit = QStringLiteral(
            "[Unit]\n"
            "Description=KAGeT touchpad gesture daemon\n"
            "PartOf=graphical-session.target\n"
            "After=graphical-session.target\n\n"
            "[Service]\n"
            "ExecStart=%1\n"
            "# Re-enable the touchpad pointer and switch the NumberPad LED off even if the daemon was killed\n"
            "ExecStopPost=%1 --restore\n"
            "Restart=on-failure\n"
            "RestartSec=2\n\n"
            "[Install]\n"
            "WantedBy=graphical-session.target\n").arg(binary);
        const QString path = userUnitPath();
        QDir().mkpath(QFileInfo(path).absolutePath());
        QFile f(path);
        if (!f.open(QIODevice::WriteOnly | QIODevice::Truncate) || f.write(unit.toUtf8()) < 0) {
            Q_EMIT notify(i18n("Could not write %1: %2", path, f.errorString()), true);
            return;
        }
        f.close();
        commands << QStringList{u"--user"_s, u"daemon-reload"_s};
    }
    const QString conflict = m_setup.value(u"conflict"_s).toString();
    if (!conflict.isEmpty()) {
        if (!stopConflict) {
            Q_EMIT notify(i18n("%1 is still running and blocks the touchpad. Stop it to use KAGeT.", conflict), true);
            return;
        }
        commands << QStringList{u"--user"_s, u"disable"_s, u"--now"_s, conflict};
    }
    commands << QStringList{u"--user"_s, u"enable"_s, u"--now"_s, u"kagetd"_s};
    m_setup.clear();
    runCommands(commands, i18n("The KAGeT daemon is installed and running. It starts automatically when you log in."));
}

void DaemonClient::openConfigFile(const QString &path)
{
    QString file = path;
    if (file.startsWith(u"~/"_s))
        file = QDir::homePath() + file.mid(1);
    if (file.isEmpty() || !QFileInfo::exists(file)) {
        Q_EMIT notify(i18n("The configuration file %1 does not exist yet. It is created when the daemon starts.", file), true);
        return;
    }
    const QString mime = QMimeDatabase().mimeTypeForFile(file).name();
    KService::Ptr editor = KApplicationTrader::preferredService(mime);
    if (!editor)
        editor = KApplicationTrader::preferredService(u"text/plain"_s);
    if (editor) {
        QString id = editor->storageId();
        if (id.endsWith(u".desktop"_s))
            id.chop(8);
        if (QProcess::startDetached(u"kstart"_s, {u"--application"_s, id, u"--url"_s, QUrl::fromLocalFile(file).toString()}))
            return;
    }
    if (!QProcess::startDetached(u"kate"_s, {file}))
        Q_EMIT notify(i18n("No text editor found to open %1.", file), true);
}

void DaemonClient::runCommands(const QList<QStringList> &commands, const QString &successText)
{
    if (commands.isEmpty()) {
        Q_EMIT notify(successText, false);
        return;
    }
    if (m_systemctl)
        return;
    auto *process = new QProcess(this);
    m_systemctl = process;
    m_startingDaemon = true;
    Q_EMIT startingDaemonChanged();
    const QStringList args = commands.first();
    const QList<QStringList> rest = commands.mid(1);
    connect(process, &QProcess::finished, this, [this, process, args, rest, successText](int exitCode, QProcess::ExitStatus status) {
        process->deleteLater();
        m_systemctl = nullptr;
        m_startingDaemon = false;
        Q_EMIT startingDaemonChanged();
        if (status != QProcess::NormalExit || exitCode != 0) {
            const QString err = QString::fromLocal8Bit(process->readAllStandardError()).trimmed();
            Q_EMIT notify(i18n("“systemctl %1” failed: %2", args.join(u' '), err.isEmpty() ? i18n("exit code %1", exitCode) : err), true);
            return;
        }
        runCommands(rest, successText);
    });
    process->start(u"systemctl"_s, args);
}

void DaemonClient::restartDaemon()
{
    runSystemctl(u"restart"_s);
}

void DaemonClient::runSystemctl(const QString &verb)
{
    if (m_demo) {
        Q_EMIT notify(i18n("Demo mode: the daemon is simulated, nothing to %1.", verb), false);
        return;
    }
    if (m_systemctl)
        return;

    auto *process = new QProcess(this);
    m_systemctl = process;
    m_startingDaemon = true;
    Q_EMIT startingDaemonChanged();

    connect(process, &QProcess::finished, this, [this, process, verb](int exitCode, QProcess::ExitStatus status) {
        process->deleteLater();
        m_startingDaemon = false;
        Q_EMIT startingDaemonChanged();
        if (status != QProcess::NormalExit || exitCode != 0) {
            QString output = QString::fromLocal8Bit(process->readAllStandardError()).trimmed();
            if (output.contains(u"not found"_s, Qt::CaseInsensitive)) {
                Q_EMIT notify(i18n("The KAGeT daemon (kagetd) is not installed. Install kagetd and its user service, then try again."), true);
                return;
            }
            if (output.isEmpty())
                output = i18n("exit code %1", exitCode);
            Q_EMIT notify(i18n("“systemctl --user %1 kagetd” failed: %2", verb, output), true);
        } else {
            Q_EMIT notify(verb == u"restart"_s ? i18n("The daemon was restarted.") : i18n("The daemon was started."), false);
        }
    });
    connect(process, &QProcess::errorOccurred, this, [this, process](QProcess::ProcessError error) {
        if (error != QProcess::FailedToStart)
            return;
        process->deleteLater();
        m_startingDaemon = false;
        Q_EMIT startingDaemonChanged();
        Q_EMIT notify(i18n("Could not run systemctl: %1", process->errorString()), true);
    });
    process->start(u"systemctl"_s, {u"--user"_s, verb, u"kagetd"_s});
}

void DaemonClient::clearError()
{
    setLastError({});
}

// ---------------------------------------------------------------------------
// Signals from the daemon

void DaemonClient::onStateChangedSignal()
{
    // Collapse bursts of StateChanged into one refresh.
    if (!m_refreshTimer) {
        m_refreshTimer = new QTimer(this);
        m_refreshTimer->setSingleShot(true);
        m_refreshTimer->setInterval(50);
        connect(m_refreshTimer, &QTimer::timeout, this, &DaemonClient::refresh);
    }
    m_refreshTimer->start();
}

void DaemonClient::onTouchesSignal(const QString &json)
{
    if (!m_live)
        return;
    const QVariantMap frame = parseJson(json).toMap();
    m_contacts = frame.value(u"contacts"_s).toList();
    Q_EMIT contactsChanged();
    Q_EMIT touchFrame(frame);
}

void DaemonClient::onGestureSignal(const QString &trigger, const QString &action)
{
    const QVariantMap t = triggerInfo(trigger);
    const QVariantMap a = actionInfo(action);
    m_lastGesture = {
        {u"trigger"_s, trigger},
        {u"action"_s, action},
        {u"triggerTitle"_s, t.value(u"longTitle"_s)},
        {u"actionLabel"_s, a.value(u"label"_s)},
        {u"actionIcon"_s, a.value(u"icon"_s)},
        {u"time"_s, QDateTime::currentMSecsSinceEpoch()},
    };
    Q_EMIT lastGestureChanged();
    Q_EMIT gestureFired(trigger, action);
}

void DaemonClient::onDiagnosisSignal(const QString &json)
{
    QVariantMap d = parseJson(json).toMap();
    if (d.isEmpty())
        return;
    const QString trigger = d.value(u"trigger"_s).toString();
    const QString action = d.value(u"action"_s).toString();
    if (!trigger.isEmpty())
        d.insert(u"triggerTitle"_s, triggerInfo(trigger).value(u"longTitle"_s));
    const QString corner = d.value(u"corner"_s).toString();
    if (!corner.isEmpty())
        d.insert(u"cornerTitle"_s, cornerName(corner));
    if (!action.isEmpty()) {
        const QVariantMap a = actionInfo(action);
        d.insert(u"actionLabel"_s, a.value(u"label"_s));
        d.insert(u"actionIcon"_s, a.value(u"icon"_s));
    }
    d.insert(u"time"_s, QDateTime::currentDateTime());
    Q_EMIT diagnosis(d);
}

// ---------------------------------------------------------------------------
// Draft editing

void DaemonClient::setDraft(const QVariantMap &draft)
{
    m_draft = draft;
    updateDirty();
    Q_EMIT draftChanged();
}

void DaemonClient::updateDirty()
{
    const bool dirty = canonicalJson(m_draft) != canonicalJson(m_config);
    if (dirty != m_dirty) {
        m_dirty = dirty;
        Q_EMIT draftChanged();
    }
}

void DaemonClient::setLastError(const QString &error)
{
    if (m_lastError == error)
        return;
    m_lastError = error;
    Q_EMIT lastErrorChanged();
}

void DaemonClient::setExternalChange(bool changed)
{
    if (m_externalChange == changed)
        return;
    m_externalChange = changed;
    Q_EMIT externalChangeChanged();
}

QVariant DaemonClient::draftValue(const QString &path, const QVariant &fallback) const
{
    const QVariant v = getPath(m_draft, path.split(u'.'));
    return v.isValid() ? v : fallback;
}

void DaemonClient::setDraftValue(const QString &path, const QVariant &value)
{
    const QStringList parts = path.split(u'.');
    const QString key = parts.last();
    const QVariant existing = getPath(m_draft, parts);

    QVariant coerced = value;
    if (isIntegerKey(key)) {
        coerced = qlonglong(std::llround(value.toDouble()));
    } else {
        switch (existing.typeId()) {
        case QMetaType::Bool:
            coerced = value.toBool();
            break;
        case QMetaType::QString:
            coerced = value.toString();
            break;
        case QMetaType::Int:
        case QMetaType::LongLong:
        case QMetaType::UInt:
        case QMetaType::ULongLong:
        case QMetaType::Double:
        case QMetaType::Float:
            coerced = std::round(value.toDouble() * 1000.0) / 1000.0;
            break;
        default:
            if (value.typeId() == QMetaType::Double || value.typeId() == QMetaType::Float)
                coerced = std::round(value.toDouble() * 1000.0) / 1000.0;
            break;
        }
    }

    if (existing.isValid() && existing == coerced && existing.typeId() == coerced.typeId())
        return;
    setPath(m_draft, parts, coerced);
    updateDirty();
    Q_EMIT draftChanged();
}

QVariantMap DaemonClient::binding(const QString &trigger) const
{
    const QVariantList bindings = m_draft.value(u"bindings"_s).toList();
    for (const QVariant &b : bindings) {
        const QVariantMap m = b.toMap();
        if (m.value(u"trigger"_s).toString() == trigger)
            return m;
    }
    return {};
}

QString DaemonClient::actionFor(const QString &trigger) const
{
    return binding(trigger).value(u"action"_s).toString();
}

int DaemonClient::boundCount() const
{
    return m_draft.value(u"bindings"_s).toList().size();
}

void DaemonClient::setBinding(const QString &trigger, const QString &action, const QString &whenRunning)
{
    QVariantList bindings = m_draft.value(u"bindings"_s).toList();
    int index = -1;
    for (int i = 0; i < bindings.size(); ++i) {
        if (bindings.at(i).toMap().value(u"trigger"_s).toString() == trigger) {
            index = i;
            break;
        }
    }

    if (action.isEmpty()) {
        if (index < 0)
            return;
        bindings.removeAt(index);
    } else {
        QVariantMap b = index >= 0 ? bindings.at(index).toMap() : QVariantMap{};
        b.insert(u"trigger"_s, trigger);
        b.insert(u"action"_s, action);
        if (action.startsWith(u"app:"_s)) {
            const QString wr = whenRunning.isEmpty() ? b.value(u"when_running"_s).toString() : whenRunning;
            b.insert(u"when_running"_s, wr.isEmpty() ? u"new"_s : wr);
        } else {
            b.remove(u"when_running"_s);
        }
        if (index >= 0)
            bindings[index] = b;
        else
            bindings.append(b);
    }
    m_draft.insert(u"bindings"_s, bindings);
    updateDirty();
    Q_EMIT draftChanged();
}

// ---------------------------------------------------------------------------
// Descriptions

QVariantMap DaemonClient::triggerInfo(const QString &trigger) const
{
    const QVariantMap entry = m_triggerIndex.value(trigger);
    QVariantMap info = entry;
    const QStringList p = trigger.split(u'.');

    QString kind;
    QString group;
    QString title;
    QString longTitle;
    QString corner;
    QString edge;
    QString direction;
    QString subgroup;
    QString badge;
    int fingers = entry.value(u"fingers"_s).toInt();

    if (p.value(0) == u"corner"_s && p.size() >= 3) {
        corner = p.at(1);
        group = u"corners"_s;
        fingers = 1;
        subgroup = cornerName(corner);
        if (p.at(2) == u"hold"_s) {
            kind = u"corner_hold"_s;
            title = i18nc("@label corner gesture", "Hold");
            badge = u"⏱"_s;
        } else if (p.at(2) == u"taphold"_s) {
            kind = u"corner_taphold"_s;
            title = i18nc("@label corner gesture", "Tap and hold");
            badge = u"•⏱"_s;
        } else {
            kind = u"corner_swipe"_s;
            direction = p.value(3);
            title = cornerSwipeTitle(direction);
            badge = directionArrow(direction);
        }
        longTitle = i18nc("@label %1 corner name, %2 gesture", "%1: %2", cornerName(corner), title.toLower());
    } else if (p.value(0) == u"sidedrag"_s && p.size() >= 3) {
        kind = u"sidedrag"_s;
        group = u"side_drag"_s;
        edge = p.at(1);
        fingers = p.at(2).toInt();
        subgroup = edgeName(edge);
        title = i18nc("@label side drag", "Drag in with %1 fingers", fingers);
        longTitle = i18nc("@label side drag, %1 edge name", "%1: drag in with %2 fingers", edgeName(edge), fingers);
        badge = i18nc("@label finger count badge, e.g. 3F ↓", "%1F %2", fingers, edgeArrow(edge));
    } else if ((p.value(0) == u"swipe"_s || p.value(0) == u"pinch"_s) && p.size() >= 3) {
        kind = p.at(0);
        group = kind;
        fingers = p.at(1).toInt();
        direction = p.at(2);
        title = multiFingerTitle(kind, direction, fingers);
        longTitle = title;
        badge = i18nc("@label finger count badge", "%1F %2", fingers, directionArrow(direction));
        subgroup = i18ncp("@title", "%1 finger", "%1 fingers", fingers);
    } else if ((p.value(0) == u"hold"_s || p.value(0) == u"tap"_s || p.value(0) == u"doubletap"_s || p.value(0) == u"taphold"_s)
               && p.size() >= 2) {
        kind = p.at(0);
        group = u"taps"_s;
        fingers = p.at(1).toInt();
        title = multiFingerTitle(kind, {}, fingers);
        longTitle = title;
        badge = i18nc("@label finger count badge", "%1F", fingers);
        subgroup = i18ncp("@title", "%1 finger", "%1 fingers", fingers);
    } else {
        kind = u"other"_s;
        group = normalizeGroup(entry.value(u"group"_s).toString());
        title = trigger;
        longTitle = trigger;
    }

    const bool known = !entry.isEmpty();
    info.insert(u"trigger"_s, trigger);
    info.insert(u"kind"_s, kind);
    info.insert(u"group"_s, group);
    info.insert(u"groupLabel"_s, groupLabel(group));
    info.insert(u"groupOrder"_s, groupOrder(group));
    info.insert(u"subgroup"_s, subgroup);
    info.insert(u"title"_s, title);
    info.insert(u"longTitle"_s, longTitle);
    info.insert(u"badge"_s, badge);
    info.insert(u"corner"_s, corner);
    info.insert(u"edge"_s, edge);
    info.insert(u"direction"_s, direction);
    info.insert(u"fingers"_s, fingers);
    info.insert(u"known"_s, known);
    const bool systemOwned = known ? entry.value(u"system_owned"_s).toBool() : isSystemOwnedBySpec(trigger);
    bool available = known ? entry.value(u"available"_s, true).toBool() : false;
    QString reason = known ? entry.value(u"reason"_s).toString() : i18n("This gesture is not reported by the daemon for this touchpad.");
    if (systemOwned && passive()) {
        // Passive mode (no exclusive grab): KDE keeps its own gestures.
        available = false;
        if (reason.isEmpty())
            reason = i18n("Used by KDE");
    }
    info.insert(u"systemOwned"_s, systemOwned);
    info.insert(u"available"_s, available);
    info.insert(u"reason"_s, reason);
    info.insert(u"description"_s, entry.value(u"description"_s).toString());
    return info;
}

QVariantMap DaemonClient::actionInfo(const QString &action) const
{
    QVariantMap info;
    info.insert(u"action"_s, action);

    QString label;
    QString category;
    QString kind;
    QString detail;
    QString icon = iconForAction(action);
    bool slider = !knownInverse(action).isEmpty();

    const auto it = m_actionIndex.constFind(action);
    if (action.isEmpty()) {
        label = i18nc("@label no binding", "Not assigned");
        kind = u"unassigned"_s;
        icon = QString();
    } else if (action == u"system"_s) {
        label = i18nc("@label action", "System default");
        kind = u"system"_s;
        category = i18nc("@title action category", "General");
    } else if (action == u"none"_s) {
        label = i18nc("@label action", "Do nothing");
        kind = u"none"_s;
        category = i18nc("@title action category", "General");
    } else if (action.startsWith(u"app:"_s)) {
        kind = u"app"_s;
        detail = action.mid(4);
        const KService::Ptr service = KService::serviceByStorageId(detail);
        if (service) {
            label = service->name();
            if (!service->icon().isEmpty())
                icon = service->icon();
        } else {
            label = detail;
        }
        category = i18nc("@title action category", "Applications");
    } else if (action.startsWith(u"key:"_s)) {
        kind = u"key"_s;
        detail = action.mid(4);
        label = i18nc("@label action, %1 key combination", "Keys %1", Helpers::prettyKeyCombo(detail));
        category = i18nc("@title action category", "Keyboard");
    } else if (action.startsWith(u"shortcut:"_s)) {
        kind = u"shortcut"_s;
        detail = action.mid(9);
        label = i18nc("@label action, %1 shortcut", "Shortcut %1", detail);
        category = i18nc("@title action category", "Keyboard");
    } else if (action.startsWith(u"cmd:"_s)) {
        kind = u"cmd"_s;
        detail = action.mid(4);
        label = i18nc("@label action, %1 shell command", "Run “%1”", detail);
        category = i18nc("@title action category", "Commands");
    } else if (action.startsWith(u"kwin:"_s)) {
        kind = u"kwin"_s;
        detail = action.mid(5);
        label = detail;
        category = u"KWin"_s;
    } else if (action.startsWith(u"global:"_s)) {
        kind = u"global"_s;
        detail = action.mid(7);
        const int colon = detail.indexOf(u':');
        if (colon > 0)
            label = i18nc("@label %1 shortcut name, %2 component", "%1 (%2)", detail.mid(colon + 1), detail.left(colon));
        else
            label = detail;
        category = i18nc("@title action category", "Global shortcuts");
    } else {
        kind = u"builtin"_s;
        label = action;
    }

    if (it != m_actionIndex.cend()) {
        const QString listed = it->value(u"label"_s).toString();
        if (!listed.isEmpty() && kind != u"unassigned"_s && kind != u"app"_s)
            label = listed;
        const QString listedCategory = it->value(u"category"_s).toString();
        if (!listedCategory.isEmpty())
            category = listedCategory;
        const QVariant s = it->value(u"slider"_s);
        if (s.isValid() && !s.isNull())
            slider = s.typeId() == QMetaType::QString ? !s.toString().isEmpty() : s.toBool();
        info.insert(u"listed"_s, true);
    } else {
        info.insert(u"listed"_s, false);
    }

    info.insert(u"label"_s, label);
    info.insert(u"category"_s, category);
    info.insert(u"kind"_s, kind);
    info.insert(u"detail"_s, detail);
    info.insert(u"icon"_s, icon);
    info.insert(u"slider"_s, slider);
    return info;
}

QString DaemonClient::multitouchLevel() const
{
    const QVariantMap device = m_state.value(u"device"_s).toMap();
    if (device.isEmpty())
        return i18n("No touchpad detected");
    const int fingers = device.value(u"max_fingers"_s).toInt();
    if (device.value(u"semi_mt"_s).toBool())
        return i18n("Semi-multitouch: multi-finger gestures are not supported");
    if (fingers >= 3)
        return i18np("Full multitouch, up to %1 finger", "Full multitouch, up to %1 fingers", fingers);
    return i18np("Limited multitouch (%1 finger): multi-finger gestures are not supported",
                 "Limited multitouch (%1 fingers): multi-finger gestures are not supported",
                 fingers);
}

QString DaemonClient::zoneTitle(const QString &zone) const
{
    if (zone.startsWith(u"corner."_s))
        return cornerName(zone.mid(7));
    if (zone.startsWith(u"side."_s)) {
        const QString e = zone.mid(5);
        if (e == u"top"_s)
            return i18nc("@title", "Top side-drag band");
        if (e == u"right"_s)
            return i18nc("@title", "Right side-drag band");
        if (e == u"bottom"_s)
            return i18nc("@title", "Bottom side-drag band");
        return i18nc("@title", "Left side-drag band");
    }
    if (zone.startsWith(u"band."_s)) {
        const QString e = zone.mid(5);
        if (e == u"top"_s)
            return i18nc("@title", "Top gesture area");
        if (e == u"right"_s)
            return i18nc("@title", "Right gesture area");
        if (e == u"bottom"_s)
            return i18nc("@title", "Bottom gesture area");
        return i18nc("@title", "Left gesture area");
    }
    if (zone == u"centre"_s)
        return i18nc("@title", "Centre: multi-finger gestures");
    return zone;
}

// ---------------------------------------------------------------------------
// JSON helpers

QVariant DaemonClient::parseJson(const QString &json, QString *error)
{
    QJsonParseError pe;
    const QJsonDocument doc = QJsonDocument::fromJson(json.toUtf8(), &pe);
    if (pe.error != QJsonParseError::NoError) {
        if (error)
            *error = pe.errorString();
        return {};
    }
    if (doc.isObject())
        return doc.object().toVariantMap();
    if (doc.isArray())
        return doc.array().toVariantList();
    return {};
}

QByteArray DaemonClient::canonicalJson(const QVariantMap &config)
{
    QVariantMap copy = config;
    QVariantList bindings = copy.value(u"bindings"_s).toList();
    for (QVariant &b : bindings) {
        QVariantMap m = b.toMap();
        const QString wr = m.value(u"when_running"_s).toString();
        if (!m.value(u"action"_s).toString().startsWith(u"app:"_s) || wr.isEmpty() || wr == u"new"_s)
            m.remove(u"when_running"_s);
        b = m;
    }
    std::stable_sort(bindings.begin(), bindings.end(), [](const QVariant &a, const QVariant &b) {
        return a.toMap().value(u"trigger"_s).toString() < b.toMap().value(u"trigger"_s).toString();
    });
    copy.insert(u"bindings"_s, bindings);
    return QJsonDocument(QJsonObject::fromVariantMap(copy)).toJson(QJsonDocument::Compact);
}
