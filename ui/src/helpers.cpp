// SPDX-License-Identifier: GPL-2.0-or-later
#include "helpers.h"

#include <KApplicationTrader>
#include <KLocalizedString>
#include <KService>

#include <QClipboard>
#include <QCollator>
#include <QDBusConnection>
#include <QFile>
#include <QGuiApplication>
#include <QHash>
#include <QKeySequence>
#include <QProcess>
#include <QRegularExpression>

#include <algorithm>
#include <vector>

#include <grp.h>
#include <unistd.h>

using namespace Qt::StringLiterals;

namespace
{
// Shifted US-layout symbols back to the key that produces them.
int unshiftUs(int key)
{
    switch (key) {
    case Qt::Key_Exclam:
        return Qt::Key_1;
    case Qt::Key_At:
        return Qt::Key_2;
    case Qt::Key_NumberSign:
        return Qt::Key_3;
    case Qt::Key_Dollar:
        return Qt::Key_4;
    case Qt::Key_Percent:
        return Qt::Key_5;
    case Qt::Key_AsciiCircum:
        return Qt::Key_6;
    case Qt::Key_Ampersand:
        return Qt::Key_7;
    case Qt::Key_Asterisk:
        return Qt::Key_8;
    case Qt::Key_ParenLeft:
        return Qt::Key_9;
    case Qt::Key_ParenRight:
        return Qt::Key_0;
    case Qt::Key_Underscore:
        return Qt::Key_Minus;
    case Qt::Key_Plus:
        return Qt::Key_Equal;
    case Qt::Key_BraceLeft:
        return Qt::Key_BracketLeft;
    case Qt::Key_BraceRight:
        return Qt::Key_BracketRight;
    case Qt::Key_Colon:
        return Qt::Key_Semicolon;
    case Qt::Key_QuoteDbl:
        return Qt::Key_Apostrophe;
    case Qt::Key_AsciiTilde:
        return Qt::Key_QuoteLeft;
    case Qt::Key_Bar:
        return Qt::Key_Backslash;
    case Qt::Key_Less:
        return Qt::Key_Comma;
    case Qt::Key_Greater:
        return Qt::Key_Period;
    case Qt::Key_Question:
        return Qt::Key_Slash;
    case Qt::Key_Backtab:
        return Qt::Key_Tab;
    default:
        return key;
    }
}

QString linuxKeyName(int key, bool keypad)
{
    if (key >= Qt::Key_A && key <= Qt::Key_Z)
        return u"KEY_"_s + QChar(u'A' + (key - Qt::Key_A));
    if (key >= Qt::Key_0 && key <= Qt::Key_9)
        return (keypad ? u"KEY_KP"_s : u"KEY_"_s) + QChar(u'0' + (key - Qt::Key_0));
    if (key >= Qt::Key_F1 && key <= Qt::Key_F24)
        return u"KEY_F%1"_s.arg(key - Qt::Key_F1 + 1);

    if (keypad) {
        switch (key) {
        case Qt::Key_Asterisk:
            return u"KEY_KPASTERISK"_s;
        case Qt::Key_Plus:
            return u"KEY_KPPLUS"_s;
        case Qt::Key_Minus:
            return u"KEY_KPMINUS"_s;
        case Qt::Key_Slash:
            return u"KEY_KPSLASH"_s;
        case Qt::Key_Period:
        case Qt::Key_Comma:
            return u"KEY_KPDOT"_s;
        case Qt::Key_Enter:
            return u"KEY_KPENTER"_s;
        default:
            break;
        }
    }

    static const QHash<int, QString> names{
        {Qt::Key_Escape, u"KEY_ESC"_s},
        {Qt::Key_Tab, u"KEY_TAB"_s},
        {Qt::Key_Backspace, u"KEY_BACKSPACE"_s},
        {Qt::Key_Return, u"KEY_ENTER"_s},
        {Qt::Key_Enter, u"KEY_KPENTER"_s},
        {Qt::Key_Insert, u"KEY_INSERT"_s},
        {Qt::Key_Delete, u"KEY_DELETE"_s},
        {Qt::Key_Pause, u"KEY_PAUSE"_s},
        {Qt::Key_Print, u"KEY_SYSRQ"_s},
        {Qt::Key_SysReq, u"KEY_SYSRQ"_s},
        {Qt::Key_Home, u"KEY_HOME"_s},
        {Qt::Key_End, u"KEY_END"_s},
        {Qt::Key_Left, u"KEY_LEFT"_s},
        {Qt::Key_Up, u"KEY_UP"_s},
        {Qt::Key_Right, u"KEY_RIGHT"_s},
        {Qt::Key_Down, u"KEY_DOWN"_s},
        {Qt::Key_PageUp, u"KEY_PAGEUP"_s},
        {Qt::Key_PageDown, u"KEY_PAGEDOWN"_s},
        {Qt::Key_Shift, u"KEY_LEFTSHIFT"_s},
        {Qt::Key_Control, u"KEY_LEFTCTRL"_s},
        {Qt::Key_Meta, u"KEY_LEFTMETA"_s},
        {Qt::Key_Super_L, u"KEY_LEFTMETA"_s},
        {Qt::Key_Super_R, u"KEY_RIGHTMETA"_s},
        {Qt::Key_Alt, u"KEY_LEFTALT"_s},
        {Qt::Key_AltGr, u"KEY_RIGHTALT"_s},
        {Qt::Key_CapsLock, u"KEY_CAPSLOCK"_s},
        {Qt::Key_NumLock, u"KEY_NUMLOCK"_s},
        {Qt::Key_ScrollLock, u"KEY_SCROLLLOCK"_s},
        {Qt::Key_Menu, u"KEY_COMPOSE"_s},
        {Qt::Key_Space, u"KEY_SPACE"_s},
        {Qt::Key_Apostrophe, u"KEY_APOSTROPHE"_s},
        {Qt::Key_Comma, u"KEY_COMMA"_s},
        {Qt::Key_Minus, u"KEY_MINUS"_s},
        {Qt::Key_Period, u"KEY_DOT"_s},
        {Qt::Key_Slash, u"KEY_SLASH"_s},
        {Qt::Key_Semicolon, u"KEY_SEMICOLON"_s},
        {Qt::Key_Equal, u"KEY_EQUAL"_s},
        {Qt::Key_BracketLeft, u"KEY_LEFTBRACE"_s},
        {Qt::Key_Backslash, u"KEY_BACKSLASH"_s},
        {Qt::Key_BracketRight, u"KEY_RIGHTBRACE"_s},
        {Qt::Key_QuoteLeft, u"KEY_GRAVE"_s},
        {Qt::Key_VolumeDown, u"KEY_VOLUMEDOWN"_s},
        {Qt::Key_VolumeUp, u"KEY_VOLUMEUP"_s},
        {Qt::Key_VolumeMute, u"KEY_MUTE"_s},
        {Qt::Key_MediaPlay, u"KEY_PLAYPAUSE"_s},
        {Qt::Key_MediaTogglePlayPause, u"KEY_PLAYPAUSE"_s},
        {Qt::Key_MediaStop, u"KEY_STOPCD"_s},
        {Qt::Key_MediaNext, u"KEY_NEXTSONG"_s},
        {Qt::Key_MediaPrevious, u"KEY_PREVIOUSSONG"_s},
        {Qt::Key_MonBrightnessUp, u"KEY_BRIGHTNESSUP"_s},
        {Qt::Key_MonBrightnessDown, u"KEY_BRIGHTNESSDOWN"_s},
        {Qt::Key_Calculator, u"KEY_CALC"_s},
    };
    return names.value(key);
}

QVariantMap check(const QString &title, bool ok, const QString &detail, bool informational = false)
{
    return {
        {u"title"_s, title},
        {u"ok"_s, ok},
        {u"detail"_s, detail},
        {u"informational"_s, informational},
    };
}
} // namespace

Helpers::Helpers(QObject *parent)
    : QObject(parent)
{
}

QVariantList Helpers::installedApps()
{
    if (m_appsLoaded)
        return m_apps;
    m_appsLoaded = true;

    const KService::List services = KApplicationTrader::query([](const KService::Ptr &service) {
        return !service->noDisplay() && !service->exec().isEmpty();
    });

    struct App {
        QString id, name, genericName, icon, comment;
    };
    QList<App> apps;
    apps.reserve(services.size());
    for (const KService::Ptr &s : services)
        apps.append({s->storageId(), s->name(), s->genericName(), s->icon(), s->comment()});

    QCollator collator;
    collator.setCaseSensitivity(Qt::CaseInsensitive);
    std::sort(apps.begin(), apps.end(), [&collator](const App &a, const App &b) {
        return collator.compare(a.name, b.name) < 0;
    });

    for (const App &a : std::as_const(apps)) {
        m_apps.append(QVariantMap{
            {u"id"_s, a.id},
            {u"name"_s, a.name},
            {u"genericName"_s, a.genericName},
            {u"icon"_s, a.icon.isEmpty() ? u"application-x-executable"_s : a.icon},
            {u"comment"_s, a.comment},
        });
    }
    return m_apps;
}

QVariantMap Helpers::appInfo(const QString &desktopId) const
{
    const KService::Ptr s = KService::serviceByStorageId(desktopId);
    if (!s)
        return {{u"id"_s, desktopId}, {u"name"_s, desktopId}, {u"icon"_s, u"application-x-executable"_s}, {u"found"_s, false}};
    return {{u"id"_s, desktopId}, {u"name"_s, s->name()}, {u"icon"_s, s->icon()}, {u"found"_s, true}};
}

bool Helpers::isModifierKey(int key) const
{
    switch (key) {
    case Qt::Key_Shift:
    case Qt::Key_Control:
    case Qt::Key_Meta:
    case Qt::Key_Alt:
    case Qt::Key_AltGr:
    case Qt::Key_Super_L:
    case Qt::Key_Super_R:
        return true;
    default:
        return false;
    }
}

QString Helpers::keyComboFromEvent(int key, int modifiers) const
{
    QStringList parts;
    const auto mods = Qt::KeyboardModifiers(modifiers);
    if (mods & Qt::ControlModifier)
        parts << u"KEY_LEFTCTRL"_s;
    if (mods & Qt::ShiftModifier)
        parts << u"KEY_LEFTSHIFT"_s;
    if (mods & Qt::AltModifier)
        parts << u"KEY_LEFTALT"_s;
    if (mods & Qt::MetaModifier)
        parts << u"KEY_LEFTMETA"_s;
    if (!isModifierKey(key)) {
        const bool keypad = mods & Qt::KeypadModifier;
        const QString name = linuxKeyName(keypad ? key : unshiftUs(key), keypad);
        if (name.isEmpty())
            return {};
        parts << name;
    }
    parts.removeDuplicates();
    return parts.join(u'+');
}

QString Helpers::shortcutFromEvent(int key, int modifiers) const
{
    const auto mods = Qt::KeyboardModifiers(modifiers) & (Qt::ControlModifier | Qt::ShiftModifier | Qt::AltModifier | Qt::MetaModifier);
    if (isModifierKey(key)) {
        QStringList parts;
        if (mods & Qt::ControlModifier)
            parts << u"Ctrl"_s;
        if (mods & Qt::AltModifier)
            parts << u"Alt"_s;
        if (mods & Qt::ShiftModifier)
            parts << u"Shift"_s;
        if (mods & Qt::MetaModifier)
            parts << u"Meta"_s;
        return parts.join(u'+');
    }
    const int base = (mods & Qt::ShiftModifier) ? unshiftUs(key) : key;
    return QKeySequence(QKeyCombination(mods, Qt::Key(base))).toString(QKeySequence::PortableText);
}

bool Helpers::isValidKeyCombo(const QString &combo) const
{
    static const QRegularExpression re(u"^KEY_[A-Z0-9_]+(\\+KEY_[A-Z0-9_]+)*$"_s);
    return re.match(combo).hasMatch();
}

QString Helpers::prettyKeyCombo(const QString &combo)
{
    static const QHash<QString, QString> names{
        {u"LEFTCTRL"_s, u"Ctrl"_s},       {u"RIGHTCTRL"_s, u"Ctrl"_s},  {u"LEFTSHIFT"_s, u"Shift"_s},  {u"RIGHTSHIFT"_s, u"Shift"_s},
        {u"LEFTALT"_s, u"Alt"_s},         {u"RIGHTALT"_s, u"AltGr"_s},  {u"LEFTMETA"_s, u"Meta"_s},    {u"RIGHTMETA"_s, u"Meta"_s},
        {u"ESC"_s, u"Esc"_s},             {u"ENTER"_s, u"Enter"_s},     {u"KPENTER"_s, u"Num Enter"_s}, {u"SPACE"_s, u"Space"_s},
        {u"BACKSPACE"_s, u"Backspace"_s}, {u"TAB"_s, u"Tab"_s},         {u"DELETE"_s, u"Del"_s},       {u"INSERT"_s, u"Ins"_s},
        {u"PAGEUP"_s, u"PgUp"_s},         {u"PAGEDOWN"_s, u"PgDn"_s},   {u"SYSRQ"_s, u"Print"_s},      {u"GRAVE"_s, u"`"_s},
        {u"MINUS"_s, u"-"_s},             {u"EQUAL"_s, u"="_s},         {u"LEFTBRACE"_s, u"["_s},      {u"RIGHTBRACE"_s, u"]"_s},
        {u"SEMICOLON"_s, u";"_s},         {u"APOSTROPHE"_s, u"'"_s},    {u"BACKSLASH"_s, u"\\"_s},     {u"COMMA"_s, u","_s},
        {u"DOT"_s, u"."_s},               {u"SLASH"_s, u"/"_s},         {u"VOLUMEUP"_s, u"Volume Up"_s}, {u"VOLUMEDOWN"_s, u"Volume Down"_s},
        {u"MUTE"_s, u"Mute"_s},           {u"PLAYPAUSE"_s, u"Play"_s},  {u"NEXTSONG"_s, u"Next"_s},    {u"PREVIOUSSONG"_s, u"Previous"_s},
    };
    QStringList out;
    const QStringList parts = combo.split(u'+', Qt::SkipEmptyParts);
    for (const QString &raw : parts) {
        QString k = raw.trimmed();
        if (k.startsWith(u"KEY_"_s))
            k = k.mid(4);
        const auto it = names.constFind(k);
        if (it != names.cend()) {
            out << it.value();
        } else if (k.size() <= 3) {
            out << k; // A, 7, F12
        } else {
            out << k.left(1) + k.mid(1).toLower();
        }
    }
    return out.join(u'+');
}

void Helpers::copyToClipboard(const QString &text) const
{
    if (QClipboard *clipboard = QGuiApplication::clipboard())
        clipboard->setText(text);
}

void Helpers::runAccessChecks()
{
    if (m_checkingAccess)
        return;
    QVariantList checks;

    checks << check(i18n("D-Bus session bus"),
                    QDBusConnection::sessionBus().isConnected(),
                    QDBusConnection::sessionBus().isConnected() ? i18n("Connected") : i18n("Not connected: the UI cannot reach the daemon."));

    const bool uinput = ::access("/dev/uinput", W_OK) == 0;
    checks << check(i18n("/dev/uinput writable"),
                    uinput,
                    uinput ? i18n("The daemon can create the virtual touchpad clone.")
                           : i18n("Not writable by this user. The daemon needs it to create the virtual touchpad clone (udev rule or the “input” group)."));

    // Touchpads listed by the kernel, and whether their event nodes are readable.
    QFile devices(u"/proc/bus/input/devices"_s);
    QStringList found;
    bool anyReadable = false;
    if (devices.open(QIODevice::ReadOnly)) {
        const QString all = QString::fromUtf8(devices.readAll());
        const QStringList blocks = all.split(u"\n\n"_s, Qt::SkipEmptyParts);
        static const QRegularExpression nameRe(u"N: Name=\"([^\"]*)\""_s);
        static const QRegularExpression eventRe(u"\\b(event\\d+)\\b"_s);
        for (const QString &block : blocks) {
            const QString name = nameRe.match(block).captured(1);
            if (!name.contains(u"touchpad"_s, Qt::CaseInsensitive))
                continue;
            const QString event = eventRe.match(block).captured(1);
            if (event.isEmpty())
                continue;
            const QByteArray node = "/dev/input/" + event.toLatin1();
            const bool readable = ::access(node.constData(), R_OK) == 0;
            anyReadable = anyReadable || readable;
            found << i18nc("@info %1 event node, %2 device name, %3 readable state",
                           "%1 (%2): %3",
                           event,
                           name,
                           readable ? i18n("readable") : i18n("not readable"));
        }
    }
    checks << check(i18n("Touchpad input device"),
                    anyReadable,
                    found.isEmpty() ? i18n("No touchpad found in /proc/bus/input/devices.") : found.join(u"\n"_s));

    bool inInputGroup = false;
    if (const group *g = ::getgrnam("input")) {
        const int n = ::getgroups(0, nullptr);
        if (n > 0) {
            std::vector<gid_t> groups(static_cast<size_t>(n));
            if (::getgroups(n, groups.data()) >= 0)
                inInputGroup = std::find(groups.begin(), groups.end(), g->gr_gid) != groups.end();
        }
    }
    checks << check(i18n("Member of the “input” group"),
                    inInputGroup,
                    inInputGroup ? i18n("Yes") : i18n("No (not needed if a udev rule grants access to the touchpad and /dev/uinput)."),
                    true);

    m_accessChecks = checks;
    m_checkingAccess = true;
    Q_EMIT accessChecksChanged();

    // systemd user unit (asynchronous).
    auto *process = new QProcess(this);
    connect(process, &QProcess::finished, this, [this, process](int, QProcess::ExitStatus) {
        process->deleteLater();
        QHash<QString, QString> props;
        const QStringList lines = QString::fromUtf8(process->readAllStandardOutput()).split(u'\n', Qt::SkipEmptyParts);
        for (const QString &line : lines) {
            const int eq = line.indexOf(u'=');
            if (eq > 0)
                props.insert(line.left(eq), line.mid(eq + 1));
        }
        const QString load = props.value(u"LoadState"_s);
        const bool loaded = load == u"loaded"_s;
        const QString detail = loaded ? i18nc("@info %1 active state, %2 sub state, %3 enablement",
                                              "%1 (%2), %3",
                                              props.value(u"ActiveState"_s),
                                              props.value(u"SubState"_s),
                                              props.value(u"UnitFileState"_s))
                                      : i18n("kagetd.service is not installed for this user (LoadState: %1).", load.isEmpty() ? i18n("unknown") : load);
        m_accessChecks << check(i18n("systemd user unit kagetd.service"), loaded, detail);
        m_checkingAccess = false;
        Q_EMIT accessChecksChanged();
    });
    connect(process, &QProcess::errorOccurred, this, [this, process](QProcess::ProcessError error) {
        if (error != QProcess::FailedToStart)
            return;
        process->deleteLater();
        m_accessChecks << check(i18n("systemd user unit kagetd.service"), false, i18n("systemctl could not be run."));
        m_checkingAccess = false;
        Q_EMIT accessChecksChanged();
    });
    process->start(u"systemctl"_s,
                   {u"--user"_s, u"show"_s, u"kagetd.service"_s, u"--property=LoadState,ActiveState,SubState,UnitFileState"_s});
}
