QT += core gui quick quickcontrols2 testlib dbus network
CONFIG += c++17 testcase
TARGET = aibox_tests
TEMPLATE = app

INCLUDEPATH += ../src

HEADERS += ../src/backend.h ../src/cli.h ../src/theme.h ../src/translator.h
SOURCES += aibox_tests.cpp ../src/backend.cpp ../src/cli.cpp ../src/theme.cpp ../src/translator.cpp
