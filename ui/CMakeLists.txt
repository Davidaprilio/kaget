# SPDX-License-Identifier: CC0-1.0
cmake_minimum_required(VERSION 3.22)

project(kaget VERSION 0.1.0 LANGUAGES CXX)

set(QT_MIN_VERSION "6.7.0")
set(KF_MIN_VERSION "6.8.0")

find_package(ECM ${KF_MIN_VERSION} REQUIRED NO_MODULE)
set(CMAKE_MODULE_PATH ${ECM_MODULE_PATH})

include(KDEInstallDirs)
include(KDECMakeSettings)
include(KDECompilerSettings NO_POLICY_SCOPE)
include(ECMSetupVersion)
include(FeatureSummary)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)

ecm_setup_version(${PROJECT_VERSION}
    VARIABLE_PREFIX KAGET
    VERSION_HEADER ${CMAKE_CURRENT_BINARY_DIR}/kaget-version.h)

find_package(Qt6 ${QT_MIN_VERSION} REQUIRED COMPONENTS Core Gui Widgets Qml Quick QuickControls2 DBus)
find_package(KF6 ${KF_MIN_VERSION} REQUIRED COMPONENTS
    Kirigami
    I18n
    CoreAddons
    Config
    DBusAddons
    Service
    ColorScheme)

qt_policy(SET QTP0001 NEW)
qt_policy(SET QTP0004 NEW)

qt_add_executable(kaget
    src/main.cpp
)

qt_add_qml_module(kaget
    URI io.github.kaget
    VERSION 1.0
    # Qt and KDE QML plugins are shared system plugins; no static import scan.
    NO_IMPORT_SCAN
    SOURCES
        src/daemonclient.h src/daemonclient.cpp
        src/demobackend.h src/demobackend.cpp
        src/helpers.h src/helpers.cpp
        src/screenshotter.h src/screenshotter.cpp
    QML_FILES
        qml/Main.qml
        qml/pages/OverviewPage.qml
        qml/pages/TouchpadMapPage.qml
        qml/pages/GesturesPage.qml
        qml/pages/SettingsPage.qml
        qml/pages/DiagnosticsPage.qml
        qml/pages/TestPage.qml
        qml/components/TouchpadMap.qml
        qml/components/FingerPreview.qml
        qml/components/DragLine.qml
        qml/components/SideDragBadges.qml
        qml/components/ZoneEditor.qml
        qml/components/TriggerRow.qml
        qml/components/ActionPickerDialog.qml
        qml/components/KeyRecorder.qml
        qml/components/MmSpinBox.qml
        qml/components/NumberDelegate.qml
        qml/components/ApplyBar.qml
        qml/components/InfoRow.qml
)

qt_add_resources(kaget "kaget_demo_data"
    PREFIX "/"
    FILES
        demo/state.json
        demo/config.json
        demo/defaults.json
        demo/triggers.json
        demo/actions.json
        demo/system_gestures.json
)

target_include_directories(kaget PRIVATE ${CMAKE_CURRENT_BINARY_DIR} src)

target_compile_definitions(kaget PRIVATE TRANSLATION_DOMAIN="kaget")

target_link_libraries(kaget PRIVATE
    Qt6::Core
    Qt6::Gui
    Qt6::Widgets
    Qt6::Qml
    Qt6::Quick
    Qt6::QuickControls2
    Qt6::DBus
    KF6::I18n
    KF6::I18nQml
    KF6::CoreAddons
    KF6::ConfigCore
    KF6::DBusAddons
    KF6::Service
    KF6::ColorScheme
)

install(TARGETS kaget ${KDE_INSTALL_TARGETS_DEFAULT_ARGS})
install(PROGRAMS io.github.kaget.desktop DESTINATION ${KDE_INSTALL_APPDIR})

feature_summary(WHAT ALL INCLUDE_QUIET_PACKAGES FATAL_ON_MISSING_REQUIRED_PACKAGES)
