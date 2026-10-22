// SPDX-License-Identifier: GPL-2.0-or-later
// kaget-panel: touchpad control panel (brightness, volume, keyboard backlight, panel size).
// Single instance on io.github.kaget.Panel; starting it again toggles the running panel.
#include "demorecorder.h"
#include "panelcontroller.h"

#include "kaget-version.h"

#include <KAboutData>
#include <KLocalizedQmlContext>
#include <KLocalizedString>

#include <QApplication>
#include <QCommandLineParser>
#include <QDBusConnection>
#include <QDBusMessage>
#include <QIcon>
#include <QQmlApplicationEngine>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QTimer>

using namespace Qt::StringLiterals;

int main(int argc, char *argv[])
{
    // a launch from the menu carries an activation token: the panel must not take the keyboard
    // focus from the window the user is working in (and then close when it gets it back)
    qunsetenv("XDG_ACTIVATION_TOKEN");
    qunsetenv("DESKTOP_STARTUP_ID");
    // render on the GUI thread: the blur region set for a frame is then committed together with
    // that frame, so the frosted background never lags behind the card while it resizes
    if (qEnvironmentVariableIsEmpty("QSG_RENDER_LOOP"))
        qputenv("QSG_RENDER_LOOP", "basic");
    QApplication app(argc, argv);
    KLocalizedString::setApplicationDomain("kaget");
    app.setQuitOnLastWindowClosed(false);

    KAboutData about(u"kaget-panel"_s,
                     i18nc("@title application name", "KAGeT Control Panel"),
                     QStringLiteral(KAGET_VERSION_STRING),
                     i18n("Touchpad control panel"),
                     KAboutLicense::Unknown);
    about.setDesktopFileName(u"io.github.kaget.panel"_s);
    KAboutData::setApplicationData(about);
    QGuiApplication::setWindowIcon(QIcon::fromTheme(u"io.github.kaget"_s, QIcon(u":/icons/sc-apps-io.github.kaget.svg"_s)));

    QCommandLineParser parser;
    about.setupCommandLine(&parser);
    QCommandLineOption toggleOption(u"toggle"_s, i18n("Show the panel, or hide it when it is open (the default)."));
    QCommandLineOption backgroundOption(u"background"_s, i18n("Start hidden."));
    QCommandLineOption demoOption(u"demo"_s, i18n("Sample values, no daemon."));
    QCommandLineOption screenshotOption(u"screenshot"_s, u"Render the panel to <file> and quit."_s, u"file"_s);
    screenshotOption.setFlags(QCommandLineOption::HiddenFromHelp);
    QCommandLineOption recordOption(u"record"_s, u"Play a scripted demo and save its frames to <dir>."_s, u"dir"_s);
    recordOption.setFlags(QCommandLineOption::HiddenFromHelp);
    parser.addOptions({toggleOption, backgroundOption, demoOption, screenshotOption, recordOption});
    parser.process(app);
    about.processCommandLine(&parser);

    const QString screenshot = parser.value(screenshotOption);
    const QString record = parser.value(recordOption);
    const bool demo = parser.isSet(demoOption) || !screenshot.isEmpty() || !record.isEmpty();
    if (!screenshot.isEmpty()) {
        QQuickWindow::setGraphicsApi(QSGRendererInterface::Software);
        QApplication::setStyle(u"Breeze"_s);
        if (QIcon::themeName().isEmpty() || QIcon::themeName() == u"hicolor"_s)
            QIcon::setThemeName(u"breeze"_s);
    }
    if (qEnvironmentVariableIsEmpty("QT_QUICK_CONTROLS_STYLE"))
        QQuickStyle::setStyle(u"org.kde.desktop"_s);

    PanelController panel(demo);
    PanelDBus dbus(&panel);
    auto bus = QDBusConnection::sessionBus();
    if (!demo) {
        bus.registerObject(u"/Panel"_s, &dbus, QDBusConnection::ExportScriptableSlots);
        if (!bus.registerService(u"io.github.kaget.Panel"_s)) {
            // already running: hand the request over and leave
            if (!parser.isSet(backgroundOption)) {
                auto msg = QDBusMessage::createMethodCall(u"io.github.kaget.Panel"_s, u"/Panel"_s, u"io.github.kaget.Panel1"_s, u"Toggle"_s);
                bus.call(msg, QDBus::Block, 2000);
            }
            return 0;
        }
    }

    QQmlApplicationEngine engine;
    KLocalization::setupLocalizedContext(&engine);
    engine.loadFromModule("io.github.kaget.panel", "Panel");
    if (engine.rootObjects().isEmpty())
        return 1;
    auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().constFirst());
    if (!window)
        return 1;

    if (!record.isEmpty()) {
        panel.setWindow(window);
        startDemoRecording(window, &panel, record);
        return app.exec();
    }

    if (!screenshot.isEmpty()) {
        panel.setWindow(window);
        panel.show();
        QTimer::singleShot(800, &app, [window, screenshot]() {
            const QImage image = window->grabWindow();
            if (image.isNull() || !image.save(screenshot))
                qWarning("kaget-panel: could not save %s", qPrintable(screenshot));
            QCoreApplication::quit();
        });
        return app.exec();
    }

    panel.setWindow(window);
    if (!parser.isSet(backgroundOption))
        panel.showSelf();
    return app.exec();
}
