// SPDX-License-Identifier: GPL-2.0-or-later
#pragma once

#include <QHash>
#include <QObject>
#include <QStringList>
#include <QTimer>
#include <QVariantMap>
#include <QtQmlIntegration>

class QQuickWindow;
class QQmlEngine;
class QJSEngine;

/// State and behaviour of the touchpad control panel. Values come from the daemon
/// (GetControls / ControlsChanged); slider moves go back through SetControl.
class PanelController : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON

    Q_PROPERTY(bool shown READ shown NOTIFY shownChanged)
    Q_PROPERTY(QStringList controls READ controls NOTIFY configChanged)
    Q_PROPERTY(QString position READ position NOTIFY configChanged)
    Q_PROPERTY(double scale READ scale WRITE setScale NOTIFY scaleChanged)
    Q_PROPERTY(QVariantMap values READ values NOTIFY valuesChanged)
    Q_PROPERTY(bool daemonRunning READ daemonRunning NOTIFY daemonRunningChanged)
    /// Touchpad panel mode: column under the finger, -1 when none.
    Q_PROPERTY(int focusColumn READ focusColumn NOTIFY focusColumnChanged)
    /// Touchpad size in mm: the panel is drawn as a miniature of it.
    Q_PROPERTY(double padWidth READ padWidth NOTIFY padSizeChanged)
    Q_PROPERTY(double padHeight READ padHeight NOTIFY padSizeChanged)
    /// The compositor blurs behind the panel (otherwise the card tints itself more).
    Q_PROPERTY(bool blurAvailable READ blurAvailable NOTIFY blurAvailableChanged)
    /// Bumped when action labels arrive; slotInfo() bindings depend on it.
    Q_PROPERTY(int catalogRevision READ catalogRevision NOTIFY catalogChanged)

public:
    explicit PanelController(bool demo, QObject *parent = nullptr);

    static PanelController *create(QQmlEngine *, QJSEngine *);
    static inline PanelController *s_instance = nullptr;

    void setWindow(QQuickWindow *window);

    bool shown() const { return m_shown; }
    QStringList controls() const { return m_controls; }
    QString position() const { return m_position; }
    double scale() const { return m_scale; }
    void setScale(double scale);
    QVariantMap values() const { return m_values; }
    bool daemonRunning() const { return m_daemonRunning; }
    int focusColumn() const { return m_focusColumn; }
    double padWidth() const { return m_padWidth; }
    double padHeight() const { return m_padHeight; }
    int catalogRevision() const { return m_catalogRevision; }
    bool blurAvailable() const { return m_blurAvailable; }

    /// A slot: `{action: bool, kind, label, icon}`; `kind` is the slider name or the action string.
    Q_INVOKABLE QVariantMap slotInfo(const QString &slot) const;
    /// A button slot was clicked.
    Q_INVOKABLE void runAction(const QString &action);

    /// Slider moved: applied right away in the panel, sent to the daemon at most every 40 ms.
    Q_INVOKABLE void setControl(const QString &kind, double percent);
    /// Save the size chosen with the scale slider.
    Q_INVOKABLE void commitScale();
    /// Pointer over the panel: no auto-hide while it is there.
    Q_INVOKABLE void setPointerInside(bool inside);
    /// Any interaction keeps the panel open (the daemon times out panel mode; without the daemon a
    /// local timer does).
    Q_INVOKABLE void keepAlive();
    /// Closed by the user here (Esc, click elsewhere): panel mode ends too.
    Q_INVOKABLE void userHide();
    /// Opened by starting the app: panel mode starts too.
    void showSelf();
    /// Where the glass card is inside the (larger, transparent) window: blur, vibrancy and the
    /// input region follow it.
    Q_INVOKABLE void setCardGeometry(qreal x, qreal y, qreal width, qreal height, qreal radius);
    /// Blur behind the card on or off. Panel.qml switches it off while the card resizes or fades
    /// (a solid tint covers the switch) and back on once it stays put, then thins the tint.
    Q_INVOKABLE void setBlurEnabled(bool enabled);

public Q_SLOTS:
    /// Window only; the daemon calls these when panel mode starts / ends.
    void show();
    void hide();
    /// Shortcut: the daemon decides (it owns panel mode), or a local toggle without it.
    void toggle();

Q_SIGNALS:
    void shownChanged();
    void configChanged();
    void scaleChanged();
    void valuesChanged();
    void daemonRunningChanged();
    void focusColumnChanged();
    void padSizeChanged();
    void catalogChanged();
    void blurAvailableChanged();
    /// The daemon ran an action from the panel (touchpad tap or click): flash its button.
    void actionRan(const QString &action);

private Q_SLOTS:
    void onControlsChanged(const QString &json);
    void onStateChanged();
    void onPanelScale(double scale);
    void onPanelFocus(int column);
    void onGesture(const QString &trigger, const QString &action);

private:
    void call(const QString &method, const QVariantList &args = {});
    void loadConfig();
    void applyControls(const QString &json);
    void applyPlacement();
    void updateBlur();
    void checkEffects();
    void flushPending();
    void setDaemonRunning(bool running);

    QQuickWindow *m_window = nullptr;
    bool m_demo = false;
    bool m_shown = false;
    bool m_pointerInside = false;
    bool m_wasActive = false;
    /// The pointer was on the panel since it opened (only then does losing focus close it).
    bool m_usedWithPointer = false;
    bool m_layerShell = false;
    bool m_daemonRunning = false;
    QStringList m_controls{QStringLiteral("brightness"), QStringLiteral("volume"), QStringLiteral("keyboard_backlight"),
                           QStringLiteral("scale")};
    QString m_position = QStringLiteral("bottom");
    QString m_placedPosition;
    double m_scale = 1.0;
    double m_autoHideS = 3.0;
    QVariantMap m_values;
    QHash<QString, double> m_pending;
    /// When each control was last moved here; daemon values for it are ignored meanwhile.
    QHash<QString, qint64> m_lastLocal;
    qint64 m_lastScaleMove = 0;
    qint64 m_lastActivitySent = 0;
    int m_focusColumn = -1;
    double m_padWidth = 128.9;
    double m_padHeight = 72.3;
    QHash<QString, QString> m_labels;
    int m_catalogRevision = 0;
    QTimer m_sendTimer;
    QTimer m_hideTimer;
    QTimer m_refreshTimer;
    /// The window is unmapped only after the fade-out.
    QTimer m_unmapTimer;
    QRectF m_card;
    qreal m_cardRadius = 0;
    /// The card moved or resized: the blur region is applied once, right before the next frame.
    bool m_blurDirty = false;
    bool m_blurAvailable = true;
    bool m_contrastAvailable = false;
    bool m_effectsChecked = false;
    bool m_blurEnabled = false;
    /// The compositor currently has a blur region from us.
    bool m_blurSent = false;
};

/// D-Bus face of the panel: io.github.kaget.Panel /Panel io.github.kaget.Panel1
class PanelDBus : public QObject
{
    Q_OBJECT
    Q_CLASSINFO("D-Bus Interface", "io.github.kaget.Panel1")

public:
    explicit PanelDBus(PanelController *panel)
        : QObject(panel)
        , m_panel(panel)
    {
    }

public Q_SLOTS:
    Q_SCRIPTABLE void Toggle() { m_panel->toggle(); }
    Q_SCRIPTABLE void Show() { m_panel->show(); }
    Q_SCRIPTABLE void Hide() { m_panel->hide(); }

private:
    PanelController *m_panel;
};
