// SPDX-License-Identifier: GPL-2.0-or-later
#include "daemonclient.h"
#include "screenshotter.h"

#include "kaget-version.h"

#include <KAboutData>
#include <KDBusService>
#include <KLocalizedQmlContext>
#include <KLocalizedString>

#include <QApplication>
#include <QCommandLineParser>
#include <QIcon>
#include <QQmlApplicationEngine>
#include <QQuickStyle>
#include <QQuickWindow>

#include <memory>

using namespace Qt::StringLiterals;

int main(int argc, char *argv[])
{
    QApplication app(argc, argv);
    KLocalizedString::setApplicationDomain("kaget");

    KAboutData about(u"kaget"_s,
                     i18nc("@title application name", "KAGeT"),
                     QStringLiteral(KAGET_VERSION_STRING),
                     i18n("KDE Advanced Gesture Touchpad"),
                     KAboutLicense::Unknown);
    about.setDesktopFileName(u"io.github.kaget"_s);
    about.setProgramLogo(QIcon::fromTheme(u"io.github.kaget"_s, QIcon(u":/icons/sc-apps-io.github.kaget.svg"_s)));
    // Reverse domain + "kaget" must not collide with the daemon's bus name
    // io.github.kaget, see KDBusService below.
    about.setOrganizationDomain("ui.kaget.github.io");
    KAboutData::setApplicationData(about);
    QGuiApplication::setWindowIcon(QIcon::fromTheme(u"io.github.kaget"_s, QIcon(u":/icons/sc-apps-io.github.kaget.svg"_s)));

    QCommandLineParser parser;
    about.setupCommandLine(&parser);
    QCommandLineOption demoOption(u"demo"_s, i18n("Run without the daemon, using bundled sample data."));
    QCommandLineOption screenshotOption(u"screenshot"_s, u"Render every page to PNG files in <dir> and quit."_s, u"dir"_s);
    screenshotOption.setFlags(QCommandLineOption::HiddenFromHelp);
    parser.addOption(demoOption);
    parser.addOption(screenshotOption);
    parser.process(app);
    about.processCommandLine(&parser);

    const bool demo = parser.isSet(demoOption);
    const QString screenshotDir = parser.value(screenshotOption);
    const bool screenshot = !screenshotDir.isEmpty();

    if (screenshot) {
        // Headless rendering: no GPU needed, Breeze look without a KDE platform theme.
        QQuickWindow::setGraphicsApi(QSGRendererInterface::Software);
        QApplication::setStyle(u"Breeze"_s);
        if (QIcon::themeName().isEmpty() || QIcon::themeName() == u"hicolor"_s)
            QIcon::setThemeName(u"breeze"_s);
    }

    if (qEnvironmentVariableIsEmpty("QT_QUICK_CONTROLS_STYLE"))
        QQuickStyle::setStyle(u"org.kde.desktop"_s);

    // Single instance for the normal app (registers io.github.kaget.ui.kaget,
    // never the daemon's io.github.kaget).
    std::unique_ptr<KDBusService> service;
    if (!demo && !screenshot)
        service = std::make_unique<KDBusService>(KDBusService::Unique);

    DaemonClient client(demo);

    QQmlApplicationEngine engine;
    KLocalization::setupLocalizedContext(&engine);
    engine.setInitialProperties({{u"screenshotMode"_s, screenshot}});
    engine.loadFromModule("io.github.kaget", "Main");
    if (engine.rootObjects().isEmpty())
        return 1;

    auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().constFirst());
    if (service && window) {
        QObject::connect(service.get(), &KDBusService::activateRequested, window, [window]() {
            window->show();
            window->raise();
            window->requestActivate();
        });
    }

    std::unique_ptr<Screenshotter> shooter;
    if (screenshot && window) {
        shooter = std::make_unique<Screenshotter>(window, screenshotDir);
        shooter->start();
    }

    return app.exec();
}
