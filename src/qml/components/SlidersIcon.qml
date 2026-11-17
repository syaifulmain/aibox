import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Dialogs
import QtQuick.Layouts

Item {
    property QtObject app: null
    id: slidersIcon
    property color color: theme.foreground
    implicitWidth: 14
    implicitHeight: 14

    Rectangle { x: 0; y: 2; width: 14; height: 1; color: slidersIcon.color }
    Rectangle { x: 2; y: 0; width: 3; height: 3; radius: 1.5; color: slidersIcon.color }
    Rectangle { x: 0; y: 7; width: 14; height: 1; color: slidersIcon.color }
    Rectangle { x: 9; y: 5; width: 3; height: 3; radius: 1.5; color: slidersIcon.color }
    Rectangle { x: 0; y: 12; width: 14; height: 1; color: slidersIcon.color }
    Rectangle { x: 5; y: 10; width: 3; height: 3; radius: 1.5; color: slidersIcon.color }
}
