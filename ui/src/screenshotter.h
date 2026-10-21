// SPDX-License-Identifier: GPL-2.0-or-later
#pragma once

#include <QList>
#include <QObject>
#include <QSize>
#include <QString>

class QQuickWindow;

/*!
 * Hidden `--screenshot <dir>` mode: renders a fixed list of UI states at a wide
 * (1280x800) and a narrow (500x800) window size, in light and dark colour
 * schemes, saves them as PNG and quits. Used for headless layout checks with
 * QT_QPA_PLATFORM=offscreen.
 */
class Screenshotter : public QObject
{
    Q_OBJECT

public:
    Screenshotter(QQuickWindow *window, const QString &directory, QObject *parent = nullptr);

    void start();

private:
    struct Step {
        QString file;
        QSize size;
        bool dark;
        QString state;
        int waitMs;
    };

    void runStep();
    void applyScheme(bool dark);

    QQuickWindow *m_window;
    QString m_directory;
    QList<Step> m_steps;
    int m_index = 0;
    int m_currentScheme = -1;
};
