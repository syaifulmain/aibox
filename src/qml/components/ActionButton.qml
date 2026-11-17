import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Dialogs
import QtQuick.Layouts

Rectangle {
    property QtObject app: null
    id: btn
    property string label: ""
    property string shortcutHint: ""
    property string variant: "" // "primary" | "danger" | "success" | "accent" | ""
    property bool primary: false
    property bool danger: false
    property bool success: false
    property bool accent: false
    property bool disabled: false
    property bool slidersIcon: false
    property int minWidth: 0
    readonly property bool hovered: btnMouse.containsMouse
    signal clicked()

    readonly property bool isPrimary: primary || variant === "primary"
    readonly property bool isDanger: danger || variant === "danger"
    readonly property bool isSuccess: success || variant === "success"
    readonly property bool isAccent: accent || variant === "accent" || variant === "theme"
    readonly property color contentColor: btn.isPrimary
                                          ? theme.accentForeground
                                          : (btn.isDanger
                                             ? theme.red
                                             : (btn.isAccent
                                                ? theme.accent
                                                : (btn.isSuccess ? theme.green : theme.foreground)))

    implicitHeight: 34
    implicitWidth: minWidth > 0 ? Math.max(minWidth, btnRow.implicitWidth + 24)
                                : (label === "?" ? 34 : btnRow.implicitWidth + 24)
    Layout.preferredHeight: implicitHeight
    Layout.preferredWidth: implicitWidth
    radius: Math.min(4, theme.radius)
    opacity: btn.disabled ? 0.4 : (btnMouse.containsMouse ? 0.9 : 1.0)
    color: btn.isPrimary
           ? theme.accent
           : (btn.isDanger
              ? (btnMouse.containsMouse
                 ? Qt.rgba(app.redColor.r, app.redColor.g, app.redColor.b, 0.22)
                 : Qt.rgba(app.redColor.r, app.redColor.g, app.redColor.b, 0.12))
              : (btn.isAccent
                 ? (btnMouse.containsMouse
                    ? Qt.rgba(app.accentColor.r, app.accentColor.g, app.accentColor.b, 0.22)
                    : Qt.rgba(app.accentColor.r, app.accentColor.g, app.accentColor.b, 0.12))
                 : (btn.isSuccess
                    ? (btnMouse.containsMouse
                       ? Qt.rgba(app.greenColor.r, app.greenColor.g, app.greenColor.b, 0.22)
                       : Qt.rgba(app.greenColor.r, app.greenColor.g, app.greenColor.b, 0.12))
                    : (btnMouse.containsMouse ? theme.lighterBackground : app.surfaceCard))))

    border.width: btn.isPrimary ? 0 : 1
    border.color: btn.isDanger
                  ? theme.red
                  : (btn.isAccent
                     ? theme.accent
                     : (btn.isSuccess
                        ? theme.green
                        : (btnMouse.containsMouse ? theme.muted : app.borderSubtle)))

    RowLayout {
        id: btnRow
        anchors.centerIn: parent
        spacing: 8

        SlidersIcon {
            app: btn.app
            visible: btn.slidersIcon
            color: btn.contentColor
            Layout.alignment: Qt.AlignVCenter
        }

        Text {
            visible: btn.label.length > 0
            text: btn.label
            color: btn.contentColor
            font.family: app.sansFont
            font.pixelSize: 12
            font.weight: btn.isPrimary ? Font.Medium : Font.Normal
        }

        Rectangle {
            visible: btn.shortcutHint.length > 0
            height: 18
            width: hintText.implicitWidth + 10
            radius: 2
            color: btn.isPrimary
                   ? Qt.rgba(0, 0, 0, 0.18)
                   : (btn.isDanger
                      ? Qt.rgba(app.redColor.r, app.redColor.g, app.redColor.b, 0.18)
                      : (btn.isAccent
                         ? Qt.rgba(app.accentColor.r, app.accentColor.g, app.accentColor.b, 0.18)
                         : (btn.isSuccess
                            ? Qt.rgba(app.greenColor.r, app.greenColor.g, app.greenColor.b, 0.18)
                            : app.surfaceInput)))
            border.width: btn.isPrimary ? 0 : 1
            border.color: btn.isDanger
                          ? Qt.rgba(app.redColor.r, app.redColor.g, app.redColor.b, 0.35)
                          : (btn.isAccent
                             ? Qt.rgba(app.accentColor.r, app.accentColor.g, app.accentColor.b, 0.35)
                             : (btn.isSuccess
                                ? Qt.rgba(app.greenColor.r, app.greenColor.g, app.greenColor.b, 0.35)
                                : app.borderSubtle))

            Text {
                id: hintText
                anchors.centerIn: parent
                text: btn.shortcutHint
                color: btn.isPrimary
                       ? theme.accentForeground
                       : (btn.isDanger
                          ? theme.red
                          : (btn.isAccent
                             ? theme.accent
                             : (btn.isSuccess ? theme.green : theme.muted)))
                font.family: app.monoFont
                font.pixelSize: 10
            }
        }
    }

    MouseArea {
        id: btnMouse
        anchors.fill: parent
        hoverEnabled: true
        enabled: !btn.disabled
        cursorShape: btn.disabled ? Qt.ForbiddenCursor : Qt.PointingHandCursor
        onClicked: btn.clicked()
    }
}
