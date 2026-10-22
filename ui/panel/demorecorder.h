// SPDX-License-Identifier: GPL-2.0-or-later
#pragma once

#include <QString>

class PanelController;
class QQuickWindow;

/// `--record <dir>`: plays a scripted demo on the panel (sample values) and saves every frame as
/// `<dir>/frame_NNNNN.png`, with the scene state (finger, caption, card) per frame in
/// `<dir>/states.jsonl`, for composing a demo video. Quits when done.
void startDemoRecording(QQuickWindow *window, PanelController *panel, const QString &dir);
