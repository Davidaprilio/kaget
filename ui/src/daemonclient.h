// SPDX-License-Identifier: GPL-2.0-or-later
#pragma once

#include <QDateTime>
#include <QHash>
#include <QObject>
#include <QPointer>
#include <QQmlEngine>
#include <QVariantList>
#include <QVariantMap>

#include <functional>

class QDBusServiceWatcher;
class QProcess;
class QTimer;
class DemoBackend;

/*!
 * DaemonClient talks to the kagetd daemon over the session bus
 * (io.github.kaget / /io/github/kaget/Daemon / io.github.kaget.Daemon1).
 *
 * All daemon calls are asynchronous. JSON replies are parsed into
 * QVariantMap/QVariantList for QML. The client keeps an editable copy of the
 * configuration ("draft") that QML edits through setDraftValue()/setBinding()
 * and that apply() sends back with SetConfig().
 *
 * In demo mode (kaget --demo) the same API is served by DemoBackend from
 * bundled sample data.
 */
class DaemonClient : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON

    Q_PROPERTY(bool demo READ demo CONSTANT)
    Q_PROPERTY(QString serviceName READ serviceName CONSTANT)
    Q_PROPERTY(QString objectPath READ objectPath CONSTANT)
    Q_PROPERTY(QString interfaceName READ interfaceName CONSTANT)

    Q_PROPERTY(bool connected READ connected NOTIFY connectedChanged)
    Q_PROPERTY(bool probing READ probing NOTIFY connectedChanged)
    Q_PROPERTY(QString serviceOwner READ serviceOwner NOTIFY connectedChanged)
    Q_PROPERTY(bool ready READ ready NOTIFY readyChanged)

    Q_PROPERTY(QVariantMap state READ state NOTIFY stateChanged)
    Q_PROPERTY(QString stateJson READ stateJson NOTIFY stateChanged)
    Q_PROPERTY(QString mode READ mode NOTIFY stateChanged)
    Q_PROPERTY(bool passive READ passive NOTIFY stateChanged)
    Q_PROPERTY(QVariantMap config READ config NOTIFY configChanged)
    Q_PROPERTY(QVariantMap draft READ draft NOTIFY draftChanged)
    Q_PROPERTY(bool dirty READ dirty NOTIFY draftChanged)
    Q_PROPERTY(bool externalChange READ externalChange NOTIFY externalChangeChanged)

    Q_PROPERTY(QVariantList triggers READ triggers NOTIFY triggersChanged)
    Q_PROPERTY(QVariantList actions READ actions NOTIFY actionsChanged)
    Q_PROPERTY(QVariantList systemGestures READ systemGestures NOTIFY systemGesturesChanged)

    Q_PROPERTY(QString lastError READ lastError NOTIFY lastErrorChanged)
    Q_PROPERTY(bool applying READ applying NOTIFY applyingChanged)
    Q_PROPERTY(bool startingDaemon READ startingDaemon NOTIFY startingDaemonChanged)

    Q_PROPERTY(bool live READ live NOTIFY liveChanged)
    Q_PROPERTY(QVariantList contacts READ contacts NOTIFY contactsChanged)
    Q_PROPERTY(QVariantMap lastGesture READ lastGesture NOTIFY lastGestureChanged)

public:
    explicit DaemonClient(bool demo, QObject *parent = nullptr);
    ~DaemonClient() override;

    static DaemonClient *instance();
    static DaemonClient *create(QQmlEngine *, QJSEngine *);

    bool demo() const { return m_demo; }
    QString serviceName() const;
    QString objectPath() const;
    QString interfaceName() const;

    bool connected() const { return m_connected; }
    bool probing() const { return m_probing; }
    QString serviceOwner() const { return m_serviceOwner; }
    bool ready() const { return m_ready; }

    QVariantMap state() const { return m_state; }
    QString stateJson() const { return m_stateJson; }
    QString mode() const { return m_state.value(QStringLiteral("mode")).toString(); }
    bool passive() const { return mode() == QLatin1String("passive"); }
    QVariantMap config() const { return m_config; }
    QVariantMap draft() const { return m_draft; }
    bool dirty() const { return m_dirty; }
    bool externalChange() const { return m_externalChange; }

    QVariantList triggers() const { return m_triggers; }
    QVariantList actions() const { return m_actions; }
    QVariantList systemGestures() const { return m_systemGestures; }

    QString lastError() const { return m_lastError; }
    bool applying() const { return m_applying; }
    bool startingDaemon() const { return m_startingDaemon; }

    bool live() const { return m_live; }
    QVariantList contacts() const { return m_contacts; }
    QVariantMap lastGesture() const { return m_lastGesture; }

    // --- daemon calls ---
    Q_INVOKABLE void refresh();
    Q_INVOKABLE void apply();
    Q_INVOKABLE void revert();
    Q_INVOKABLE void resetToDefaults(bool includeBindings);
    Q_INVOKABLE void runAction(const QString &action);
    Q_INVOKABLE void setDaemonEnabled(bool enabled);
    Q_INVOKABLE void togglePause();
    Q_INVOKABLE void setLive(bool live);
    Q_INVOKABLE void startDaemon();
    Q_INVOKABLE void restartDaemon();
    // Installs the per-user service if needed, optionally stops the conflicting old
    // NumberPad service, then enables and starts kagetd. Called after the setup dialog.
    Q_INVOKABLE void installAndStart(bool stopConflict);
    // Opens the file in the default text editor (TOML, then plain text), falling back to Kate.
    Q_INVOKABLE void openConfigFile(const QString &path);
    Q_INVOKABLE void clearError();

    // --- draft editing ---
    Q_INVOKABLE QVariant draftValue(const QString &path, const QVariant &fallback = {}) const;
    Q_INVOKABLE void setDraftValue(const QString &path, const QVariant &value);
    Q_INVOKABLE QVariantMap binding(const QString &trigger) const;
    Q_INVOKABLE QString actionFor(const QString &trigger) const;
    Q_INVOKABLE void setBinding(const QString &trigger, const QString &action, const QString &whenRunning = {});
    Q_INVOKABLE int boundCount() const;

    // --- descriptive helpers ---
    Q_INVOKABLE QVariantMap triggerInfo(const QString &trigger) const;
    Q_INVOKABLE QVariantMap actionInfo(const QString &action) const;
    Q_INVOKABLE QString multitouchLevel() const;
    Q_INVOKABLE QString zoneTitle(const QString &zone) const;

Q_SIGNALS:
    void connectedChanged();
    void readyChanged();
    void stateChanged();
    void configChanged();
    void draftChanged();
    void externalChangeChanged();
    void triggersChanged();
    void actionsChanged();
    void systemGesturesChanged();
    void lastErrorChanged();
    void applyingChanged();
    void startingDaemonChanged();
    void liveChanged();
    void contactsChanged();
    void lastGestureChanged();

    void gestureFired(const QString &trigger, const QString &action);
    /// What became of a touch (daemon `Diagnosis`), with `triggerTitle`, `cornerTitle`,
    /// `actionLabel` and `actionIcon` added.
    void diagnosis(const QVariantMap &d);
    void touchFrame(const QVariantMap &frame);
    void applied(bool ok, const QString &error);
    void actionTested(const QString &action, bool ok);
    void notify(const QString &text, bool isError);
    // Start needs confirmation: {unitInstalled, binary, conflict}. binary is empty if not found.
    void setupNeeded(const QVariantMap &info);

private Q_SLOTS:
    void onStateChangedSignal();
    void onTouchesSignal(const QString &json);
    void onGestureSignal(const QString &trigger, const QString &action);
    void onDiagnosisSignal(const QString &json);

private:
    using ReplyFn = std::function<void(const QVariant &)>;
    using ErrorFn = std::function<void(const QString &)>;

    void callDaemon(const QString &method, const QVariantList &args, ReplyFn onReply, ErrorFn onError = {});
    void setConnected(bool connected, const QString &owner);
    void fetchState();
    void fetchConfig(bool adoptIntoDraft);
    void fetchLists();
    void setDraft(const QVariantMap &draft);
    void updateDirty();
    void setLastError(const QString &error);
    void setExternalChange(bool changed);
    void runSystemctl(const QString &verb);
    void markLoaded(int part);

    static QVariant parseJson(const QString &json, QString *error = nullptr);
    static QByteArray canonicalJson(const QVariantMap &config);

    const bool m_demo;
    DemoBackend *m_demoBackend = nullptr;
    QDBusServiceWatcher *m_watcher = nullptr;
    QPointer<QProcess> m_systemctl;
    QTimer *m_refreshTimer = nullptr;

    bool m_connected = false;
    bool m_probing = true;
    QString m_serviceOwner;
    bool m_ready = false;
    int m_loadedParts = 0;

    QVariantMap m_state;
    QString m_stateJson;
    QVariantMap m_config;
    QVariantMap m_draft;
    bool m_dirty = false;
    bool m_externalChange = false;

    QVariantList m_triggers;
    QHash<QString, QVariantMap> m_triggerIndex;
    QVariantList m_actions;
    QHash<QString, QVariantMap> m_actionIndex;
    QVariantList m_systemGestures;

    QString m_lastError;
    bool m_applying = false;
    bool m_startingDaemon = false;
    QVariantMap m_setup;

    QVariantMap inspectSetup() const;
    void runCommands(const QList<QStringList> &commands, const QString &successText);

    bool m_live = false;
    QVariantList m_contacts;
    QVariantMap m_lastGesture;

    static DaemonClient *s_instance;
};
