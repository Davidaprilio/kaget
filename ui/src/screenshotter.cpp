// SPDX-License-Identifier: GPL-2.0-or-later
#include "screenshotter.h"

#include <KColorSchemeManager>

#include <QCoreApplication>
#include <QEvent>
#include <QDir>
#include <QIcon>
#include <QImage>
#include <QQuickWindow>
#include <QTimer>

using namespace Qt::StringLiterals;

Screenshotter::Screenshotter(QQuickWindow *window, const QString &directory, QObject *parent)
    : QObject(parent)
    , m_window(window)
    , m_directory(directory)
{
    const QSize wide(1280, 800);
    const QSize narrow(500, 800);
    constexpr int wait = 900;

    // clang-format off
    m_steps = {
        {u"wide-01-overview"_s,          wide,   false, u"overview"_s,                 wait},
        {u"wide-02-map"_s,               wide,   false, u"map"_s,                      wait},
        {u"wide-03-map-corner"_s,        wide,   false, u"map:corner.top_right"_s,     wait},
        {u"wide-04-map-side"_s,          wide,   false, u"map:side.top"_s,             wait},
        {u"wide-05-map-centre"_s,        wide,   false, u"map:centre"_s,               wait},
        {u"wide-06-map-numberpad"_s,     wide,   false, u"map-numberpad"_s,            wait},
        {u"wide-07-map-live"_s,          wide,   false, u"map-live"_s,                 4500},
        {u"wide-08-map-line-hover"_s,    wide,   false, u"map-hover"_s,                wait},
        {u"wide-09-gestures"_s,          wide,   false, u"gestures"_s,                 wait},
        {u"wide-10-gestures-override"_s, wide,   false, u"gestures-override"_s,        wait},
        {u"wide-10b-gestures-swipe"_s,   wide,   false, u"gestures:swipe"_s,           wait},
        {u"wide-10c-gestures-swipe-override"_s, wide, false, u"gestures-override:swipe"_s, wait},
        {u"wide-10d-gestures-taps"_s,    wide,   false, u"gestures:taps"_s,            wait},
        {u"wide-11-picker"_s,            wide,   false, u"picker:corner.top_left.hold"_s, wait},
        {u"wide-12-picker-app"_s,        wide,   false, u"picker-app:corner.top_left.hold"_s, wait},
        {u"wide-13-settings"_s,          wide,   false, u"settings"_s,                 wait},
        {u"wide-14-diagnostics"_s,       wide,   false, u"diagnostics"_s,              1500},
        {u"wide-15-unsaved"_s,           wide,   false, u"unsaved"_s,                  wait},
        {u"wide-16-apply-error"_s,       wide,   false, u"apply-error"_s,              wait},
        {u"wide-17-test"_s,              wide,   false, u"test"_s,                     4500},
        {u"wide-20-map-side-on"_s,       wide,   false, u"map-side-on"_s,              wait},
        {u"wide-21-map-corners-off"_s,   wide,   false, u"map-corners-off"_s,          wait},
        {u"wide-18-setup"_s,             wide,   false, u"setup"_s,                    wait},
        {u"wide-19-setup-missing"_s,     wide,   false, u"setup-missing"_s,            wait},
        {u"tall-01-diagnostics"_s,       QSize(1100, 2300), false, u"diagnostics"_s,   1500},
        {u"tall-02-settings"_s,          QSize(1100, 2300), false, u"settings"_s,      wait},
        {u"tall-03-overview"_s,          QSize(1100, 1600), false, u"overview"_s,      wait},
        {u"dark-wide-01-overview"_s,     wide,   true,  u"overview"_s,                 wait},
        {u"dark-wide-02-map-corner"_s,   wide,   true,  u"map:corner.top_left"_s,      wait},
        {u"dark-wide-03-gestures"_s,     wide,   true,  u"gestures-override"_s,        wait},
        {u"dark-wide-04-settings"_s,     wide,   true,  u"settings"_s,                 wait},
        {u"narrow-01-overview"_s,        narrow, false, u"overview"_s,                 wait},
        {u"narrow-02-map"_s,             narrow, false, u"map"_s,                      wait},
        {u"narrow-03-map-corner"_s,      narrow, false, u"map:corner.top_right"_s,     wait},
        {u"narrow-04-gestures"_s,        narrow, false, u"gestures-override:pinch"_s,  wait},
        {u"narrow-05-picker"_s,          narrow, false, u"picker:sidedrag.top.3"_s,    wait},
        {u"narrow-06-settings"_s,        narrow, false, u"settings"_s,                 wait},
        {u"narrow-07-diagnostics"_s,     narrow, false, u"diagnostics"_s,              1500},
        {u"narrow-08-drawer"_s,          narrow, false, u"drawer"_s,                   wait},
        {u"narrow-09-test"_s,            narrow, false, u"test"_s,                     4500},
        {u"dark-narrow-01-map"_s,        narrow, true,  u"map:side.left"_s,            wait},
    };
    // clang-format on
}

void Screenshotter::start()
{
    QDir().mkpath(m_directory);
    QTimer::singleShot(1200, this, &Screenshotter::runStep);
}

void Screenshotter::applyScheme(bool dark)
{
    if (m_currentScheme == int(dark))
        return;
    m_currentScheme = int(dark);
    KColorSchemeManager *manager = KColorSchemeManager::instance();
    manager->setAutosaveChanges(false);
    manager->activateSchemeId(dark ? u"BreezeDark"_s : u"BreezeLight"_s);
    QIcon::setThemeName(dark ? u"breeze-dark"_s : u"breeze"_s);
}

void Screenshotter::runStep()
{
    if (m_index >= m_steps.size()) {
        QMetaObject::invokeMethod(m_window, "prepareScreenshot", Q_ARG(QString, u"reset"_s));
        qInfo("kaget: wrote %lld screenshots to %s", qlonglong(m_steps.size()), qPrintable(m_directory));
        QCoreApplication::exit(0);
        return;
    }

    const Step step = m_steps.at(m_index);
    applyScheme(step.dark);
    m_window->resize(step.size);
    QMetaObject::invokeMethod(m_window, "prepareScreenshot", Q_ARG(QString, u"reset"_s));
    QMetaObject::invokeMethod(m_window, "prepareScreenshot", Q_ARG(QString, step.state));

    QTimer::singleShot(step.waitMs, this, [this, step]() {
        // The offscreen pointer sits at (0, 0) over the drawer button; drop hover state.
        QEvent leave(QEvent::Leave);
        QCoreApplication::sendEvent(m_window, &leave);
        const QImage image = m_window->grabWindow();
        const QString path = QDir(m_directory).filePath(step.file + u".png"_s);
        if (image.isNull() || !image.save(path))
            qWarning("kaget: could not save %s", qPrintable(path));
        ++m_index;
        runStep();
    });
}
