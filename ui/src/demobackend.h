// SPDX-License-Identifier: GPL-2.0-or-later
#pragma once

#include <QElapsedTimer>
#include <QJsonArray>
#include <QJsonObject>
#include <QObject>
#include <QTimer>
#include <QVariant>

/*!
 * DemoBackend answers the io.github.kaget.Daemon1 methods from bundled sample
 * data (ui/demo/<name>.json, compiled into the qrc) so that `kaget --demo` works
 * without the daemon. SetConfig only updates the in-memory copy and SetLive
 * plays a scripted finger animation through the Touches signal.
 */
class DemoBackend : public QObject
{
    Q_OBJECT

public:
    explicit DemoBackend(QObject *parent = nullptr);

    /// Executes a daemon method. Returns the D-Bus return value (string JSON,
    /// bool) or an invalid QVariant for methods without a return value.
    QVariant call(const QString &method, const QVariantList &args);

Q_SIGNALS:
    void touches(const QString &json);
    void gesture(const QString &trigger, const QString &action);
    /// One report per scripted touch, like the daemon's `Diagnosis` signal.
    void diagnosis(const QString &json);
    void stateChanged();

private:
    struct Contact {
        int slot;
        double x;
        double y;
        QString role;
    };

    void tick();
    QList<Contact> contactsAt(qint64 t) const;
    void fireGesturesBetween(qint64 from, qint64 to);
    QString bindingFor(const QString &trigger) const;
    QString validate(const QJsonObject &config) const;
    QString stateJson() const;

    QJsonObject m_state;
    QJsonObject m_config;
    QJsonObject m_defaults;
    QJsonArray m_triggers;
    QJsonArray m_actions;
    QJsonArray m_systemGestures;

    QTimer m_timer;
    QElapsedTimer m_clock;
    qint64 m_lastT = 0;
};
