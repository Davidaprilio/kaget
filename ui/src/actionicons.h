// SPDX-License-Identifier: GPL-2.0-or-later
// Theme icon for an action string; shared by the settings app and the control panel.
#pragma once

#include <QHash>
#include <QString>

inline QString iconForAction(const QString &action)
{
    static const QHash<QString, QString> icons{
        {QStringLiteral("none"), QStringLiteral("dialog-cancel")},
        {QStringLiteral("system"), QStringLiteral("preferences-desktop-gestures-touch")},
        {QStringLiteral("volume.up"), QStringLiteral("audio-volume-high")},
        {QStringLiteral("volume.down"), QStringLiteral("audio-volume-low")},
        {QStringLiteral("volume.mute"), QStringLiteral("audio-volume-muted")},
        {QStringLiteral("brightness.up"), QStringLiteral("brightness-high")},
        {QStringLiteral("brightness.down"), QStringLiteral("brightness-low")},
        {QStringLiteral("keyboard_backlight.up"), QStringLiteral("input-keyboard-brightness")},
        {QStringLiteral("keyboard_backlight.down"), QStringLiteral("input-keyboard-brightness")},
        {QStringLiteral("media.play_pause"), QStringLiteral("media-playback-start")},
        {QStringLiteral("media.next"), QStringLiteral("media-skip-forward")},
        {QStringLiteral("media.previous"), QStringLiteral("media-skip-backward")},
        {QStringLiteral("numberpad.toggle"), QStringLiteral("accessories-calculator")},
        {QStringLiteral("numberpad.brightness_cycle"), QStringLiteral("brightness-high")},
        {QStringLiteral("numberpad.brightness_up"), QStringLiteral("brightness-high")},
        {QStringLiteral("numberpad.brightness_down"), QStringLiteral("brightness-low")},
        {QStringLiteral("kaget.pause"), QStringLiteral("media-playback-pause")},
        {QStringLiteral("kaget.panel"), QStringLiteral("view-media-equalizer")},
        {QStringLiteral("kwin:Overview"), QStringLiteral("view-app-grid-symbolic")},
        {QStringLiteral("kwin:ExposeAll"), QStringLiteral("view-grid-symbolic")},
        {QStringLiteral("kwin:Expose"), QStringLiteral("view-grid-symbolic")},
        {QStringLiteral("kwin:Show Desktop"), QStringLiteral("desktop")},
        {QStringLiteral("kwin:Window Maximize"), QStringLiteral("window-maximize")},
        {QStringLiteral("kwin:Window Minimize"), QStringLiteral("window-minimize")},
        {QStringLiteral("kwin:Switch One Desktop to the Left"), QStringLiteral("go-previous-view")},
        {QStringLiteral("kwin:Switch One Desktop to the Right"), QStringLiteral("go-next-view")},
        {QStringLiteral("kwin:Window One Desktop to the Left"), QStringLiteral("arrow-left")},
        {QStringLiteral("kwin:Window One Desktop to the Right"), QStringLiteral("arrow-right")},
        {QStringLiteral("global:plasmashell:activate application launcher"), QStringLiteral("start-here-kde-plasma")},
        {QStringLiteral("global:plasmashell:toggle do not disturb"), QStringLiteral("notifications-disabled")},
    };
    const auto it = icons.constFind(action);
    if (it != icons.cend())
        return it.value();
    if (action.startsWith(QStringLiteral("system:")))
        return QStringLiteral("preferences-desktop-gestures-touch");
    if (action.startsWith(QStringLiteral("key:")) || action.startsWith(QStringLiteral("shortcut:")))
        return QStringLiteral("input-keyboard");
    if (action.startsWith(QStringLiteral("cmd:")))
        return QStringLiteral("utilities-terminal");
    if (action.startsWith(QStringLiteral("kwin:")))
        return QStringLiteral("kwin");
    if (action.startsWith(QStringLiteral("global:")))
        return QStringLiteral("preferences-desktop-keyboard-shortcut");
    if (action.startsWith(QStringLiteral("numberpad.")))
        return QStringLiteral("accessories-calculator");
    if (action.startsWith(QStringLiteral("app:")))
        return QStringLiteral("application-x-executable");
    return QStringLiteral("system-run");
}
