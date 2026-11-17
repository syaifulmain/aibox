QT += core gui quick quickcontrols2 testlib dbus network
CONFIG += c++17 testcase
TARGET = aibox_tests
TEMPLATE = app

INCLUDEPATH += ../src/cpp

HEADERS += ../src/cpp/backend.h ../src/cpp/backend_helpers.h ../src/cpp/cli.h ../src/cpp/theme.h ../src/cpp/translator.h
SOURCES += aibox_tests.cpp \
    ../src/cpp/backend.cpp \
    ../src/cpp/backend_config.cpp \
    ../src/cpp/backend_backup.cpp \
    ../src/cpp/backend_demo.cpp \
    ../src/cpp/backend_env.cpp \
    ../src/cpp/backend_health.cpp \
    ../src/cpp/backend_network.cpp \
    ../src/cpp/backend_provision.cpp \
    ../src/cpp/backend_secret.cpp \
    ../src/cpp/backend_session.cpp \
    ../src/cpp/backend_snapshot.cpp \
    ../src/cpp/backend_workspace.cpp \
    ../src/cpp/cli.cpp \
    ../src/cpp/theme.cpp \
    ../src/cpp/translator.cpp
