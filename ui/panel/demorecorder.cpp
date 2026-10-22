// SPDX-License-Identifier: GPL-2.0-or-later
#include "demorecorder.h"
#include "panelcontroller.h"

#include <QCoreApplication>
#include <QDir>
#include <QElapsedTimer>
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QQuickWindow>
#include <QTimer>

#include <algorithm>
#include <cmath>
#include <memory>

using namespace Qt::StringLiterals;

namespace
{
/// One finger on the touchpad: down/up times, column (-1: the top-right corner), and a vertical
/// move from fy0 to fy1 (fractions of the pad height, 0 = top) between move0 and move1.
struct Touch {
    double down, up;
    int column;
    double fy0, fy1, move0, move1;
};

const Touch kTouches[] = {
    {0.60, 1.30, -1, 0.08, 0.08, 0, 0}, // hold the corner: the panel opens
    {1.60, 3.30, 1, 0.78, 0.36, 1.75, 3.10}, // volume up
    {3.60, 5.05, 0, 0.30, 0.62, 3.75, 4.90}, // brightness down
    {5.40, 6.85, 3, 0.72, 0.56, 5.55, 6.70}, // panel size up
    {7.25, 7.42, 2, 0.55, 0.55, 0, 0}, // tap the play / pause button
};
constexpr double kOpenAt = 0.92; // corner hold time 320 ms after touching
constexpr double kHideAt = 8.80;
constexpr double kEnd = 9.80;
constexpr double kDeadZone = 0.028; // ~2 mm of the pad height
constexpr double kGain = 125.0; // percent per pad height (PANEL_GAIN 1.25)
constexpr int kColumns = 4;

double smooth(double a, double b, double t0, double t1, double t)
{
    if (t1 <= t0 || t <= t0)
        return t <= t0 ? a : b;
    if (t >= t1)
        return b;
    double x = (t - t0) / (t1 - t0);
    x = x * x * (3 - 2 * x);
    return a + (b - a) * x;
}

QString caption(double t)
{
    if (t < 1.45)
        return u"Hold a corner to open the panel"_s;
    if (t < 5.25)
        return u"Slide a finger up or down in a column"_s;
    if (t < 7.10)
        return u"Resize the panel live"_s;
    if (t < 8.60)
        return u"Tap a button column to run any action"_s;
    return u"It closes by itself: back to a normal touchpad"_s;
}

struct Recorder {
    QQuickWindow *window;
    PanelController *panel;
    QDir dir;
    QFile states;
    QElapsedTimer clock;
    QTimer timer;
    int frame = 0;
    bool opened = false, hidden = false, flashed = false;
    double lastScale = 1.0;
};
}

void startDemoRecording(QQuickWindow *window, PanelController *panel, const QString &path)
{
    auto rec = std::make_shared<Recorder>();
    rec->window = window;
    rec->panel = panel;
    rec->dir = QDir(path);
    rec->dir.mkpath(u"."_s);
    rec->states.setFileName(rec->dir.filePath(u"states.jsonl"_s));
    if (!rec->states.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        qWarning("kaget-panel: cannot write %s", qPrintable(rec->states.fileName()));
        QCoreApplication::exit(1);
        return;
    }

    panel->setForceGlass(true);
    panel->setScale(1.0);
    panel->setControl(u"volume"_s, 23);
    panel->setControl(u"brightness"_s, 100);
    window->show(); // the card stays invisible until the panel "opens"

    rec->timer.setInterval(33);
    QObject::connect(&rec->timer, &QTimer::timeout, window, [rec]() {
        const double t = rec->clock.elapsed() / 1000.0;
        PanelController *p = rec->panel;

        if (!rec->opened && t >= kOpenAt) {
            rec->opened = true;
            p->show();
        }
        if (!rec->hidden && t >= kHideAt) {
            rec->hidden = true;
            p->hide();
        }

        // the finger on the pad, and what it does to the panel
        bool down = false;
        double fx = 0, fy = 0, ring = 0;
        int column = -1;
        for (const Touch &tc : kTouches) {
            if (t < tc.down || t >= tc.up)
                continue;
            down = true;
            column = tc.column;
            fx = tc.column < 0 ? 0.965 : (tc.column + 0.5) / kColumns;
            fy = smooth(tc.fy0, tc.fy1, tc.move0, tc.move1, t);
            if (tc.column < 0)
                ring = std::clamp((t - tc.down) / (kOpenAt - tc.down), 0.0, 1.0);
            const double up = std::max(0.0, tc.fy0 - fy - kDeadZone);
            const double downMove = std::max(0.0, fy - tc.fy0 - kDeadZone);
            if (tc.column == 1)
                p->setControl(u"volume"_s, std::min(100.0, 23 + up * kGain));
            else if (tc.column == 0)
                p->setControl(u"brightness"_s, std::max(1.0, 100 - downMove * kGain));
            else if (tc.column == 3) {
                // panel size moves in 5 % steps, like the daemon sends them
                const double s = std::round((1.0 + up * kGain / 100.0 * 1.5) * 20.0) / 20.0;
                if (s != rec->lastScale) {
                    rec->lastScale = s;
                    p->setScale(s);
                }
            }
        }
        if (!rec->flashed && t >= kTouches[4].up) {
            rec->flashed = true;
            p->flashForDemo(u"media.play_pause"_s);
        }
        p->setFocusColumnForDemo(down && column >= 0 && rec->opened ? column : -1);

        const QImage image = rec->window->isVisible() ? rec->window->grabWindow() : QImage();
        const QString name = u"frame_%1.png"_s.arg(rec->frame, 5, 10, QLatin1Char('0'));
        if (!image.isNull())
            image.save(rec->dir.filePath(name));

        const QRectF card = p->cardRect();
        const QJsonObject line{
            {u"i"_s, rec->frame},
            {u"t"_s, t},
            {u"frame"_s, image.isNull() ? QString() : name},
            {u"dpr"_s, rec->window->devicePixelRatio()},
            {u"finger"_s, down},
            {u"fx"_s, fx},
            {u"fy"_s, fy},
            {u"column"_s, column},
            {u"ring"_s, ring},
            {u"open"_s, rec->opened && !rec->hidden},
            {u"glass"_s, p->blurShown()},
            {u"card"_s, QJsonArray{card.x(), card.y(), card.width(), card.height()}},
            {u"caption"_s, caption(t)},
        };
        rec->states.write(QJsonDocument(line).toJson(QJsonDocument::Compact) + '\n');
        ++rec->frame;

        if (t >= kEnd) {
            rec->timer.stop();
            rec->states.close();
            QCoreApplication::quit();
        }
    });
    rec->clock.start();
    rec->timer.start();
}
