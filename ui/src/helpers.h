// SPDX-License-Identifier: GPL-2.0-or-later
#pragma once

#include <QObject>
#include <QQmlEngine>
#include <QVariantList>
#include <QVariantMap>

/*!
 * Helpers exposes local, daemon-independent utilities to QML: the list of
 * installed applications (KService), key-combination conversion for the
 * key:/shortcut: action editors, local access checks for the Diagnostics page
 * and the clipboard.
 */
class Helpers : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON

    Q_PROPERTY(QVariantList accessChecks READ accessChecks NOTIFY accessChecksChanged)
    Q_PROPERTY(bool checkingAccess READ checkingAccess NOTIFY accessChecksChanged)

public:
    explicit Helpers(QObject *parent = nullptr);

    QVariantList accessChecks() const { return m_accessChecks; }
    bool checkingAccess() const { return m_checkingAccess; }

    /// Applications that can be launched with app:<desktop id>.
    Q_INVOKABLE QVariantList installedApps();
    Q_INVOKABLE QVariantMap appInfo(const QString &desktopId) const;

    /// Linux key-code combination ("KEY_LEFTCTRL+KEY_T") for a Qt key event.
    Q_INVOKABLE QString keyComboFromEvent(int key, int modifiers) const;
    /// Human shortcut ("Ctrl+Alt+T", US layout) for a Qt key event.
    Q_INVOKABLE QString shortcutFromEvent(int key, int modifiers) const;
    Q_INVOKABLE bool isModifierKey(int key) const;
    Q_INVOKABLE bool isValidKeyCombo(const QString &combo) const;
    Q_INVOKABLE QString prettyKey(const QString &combo) const { return prettyKeyCombo(combo); }

    Q_INVOKABLE void runAccessChecks();
    Q_INVOKABLE void copyToClipboard(const QString &text) const;

    static QString prettyKeyCombo(const QString &combo);

Q_SIGNALS:
    void accessChecksChanged();

private:
    QVariantList m_apps;
    bool m_appsLoaded = false;
    QVariantList m_accessChecks;
    bool m_checkingAccess = false;
};
