import QtQuick

// Monochrome sliders glyph used by the configuration button.
Item {
    id: slidersIcon
    property color color: theme.textPrimary
    implicitWidth: 14
    implicitHeight: 14

    Rectangle { x: 0; y: 2; width: 14; height: 1; color: slidersIcon.color }
    Rectangle { x: 2; y: 0; width: 3; height: 3; radius: 1.5; color: slidersIcon.color }
    Rectangle { x: 0; y: 7; width: 14; height: 1; color: slidersIcon.color }
    Rectangle { x: 9; y: 5; width: 3; height: 3; radius: 1.5; color: slidersIcon.color }
    Rectangle { x: 0; y: 12; width: 14; height: 1; color: slidersIcon.color }
    Rectangle { x: 5; y: 10; width: 3; height: 3; radius: 1.5; color: slidersIcon.color }
}
