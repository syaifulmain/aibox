QT += core gui quick quickcontrols2 dbus network
CONFIG += c++17 console
CONFIG -= app_bundle
TARGET = golden_capture
TEMPLATE = app

INCLUDEPATH += ../../src/cpp

HEADERS += \
    ../../src/cpp/backend.h \
    ../../src/cpp/backend_helpers.h \
    ../../src/cpp/cli.h \
    ../../src/cpp/rust_bridge.h \
    ../../src/cpp/theme.h \
    ../../src/cpp/translator.h

SOURCES += \
    main.cpp \
    ../../src/cpp/backend.cpp \
    ../../src/cpp/backend_config.cpp \
    ../../src/cpp/backend_backup.cpp \
    ../../src/cpp/backend_demo.cpp \
    ../../src/cpp/backend_env.cpp \
    ../../src/cpp/backend_health.cpp \
    ../../src/cpp/backend_network.cpp \
    ../../src/cpp/backend_provision.cpp \
    ../../src/cpp/backend_secret.cpp \
    ../../src/cpp/backend_session.cpp \
    ../../src/cpp/backend_snapshot.cpp \
    ../../src/cpp/backend_workspace.cpp \
    ../../src/cpp/cli.cpp \
    ../../src/cpp/theme.cpp \
    ../../src/cpp/translator.cpp

RUST_TARGET = $$PWD/../../rust/target/release
INCLUDEPATH += $$PWD/../../rust/target/cxxbridge
LIBS += $$RUST_TARGET/libaibox_ffi.a -ldl -lpthread -lm
PRE_TARGETDEPS += $$RUST_TARGET/libaibox_ffi.a
