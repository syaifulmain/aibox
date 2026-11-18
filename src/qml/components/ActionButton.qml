import QtQuick
import QtQuick.Layouts

// Flat action button with an optional sliders icon.
Rectangle {
    id: btn
    property string label: ""
    property string variant: "default" // "default" | "primary" | "danger" | "accent"
    property bool compact: false
    property bool disabled: false
    property bool slidersIcon: false
    property int minWidth: 0
    readonly property bool hovered: btnMouse.containsMouse
    signal clicked()

    readonly property string tone: btn.variant === "primary" ? "primary"
                                 : btn.variant === "danger" ? "danger"
                                 : btn.variant === "accent" ? "accent"
                                 : "default"
    readonly property bool isPrimary: tone === "primary"
    readonly property bool isDefault: tone === "default"
    readonly property bool isHovered: btnMouse.containsMouse && !btn.disabled
    readonly property bool isIconOnly: btn.label === "?" || (btn.slidersIcon && btn.label.length === 0)
    readonly property color contentColor: btn.disabled ? theme.textDisabled
                                      : btn.isPrimary ? theme.textOnAccent
                                      : btn.isDefault ? theme.textPrimary
                                      : btn.tone === "danger" ? theme.dangerText
                                      : theme.accentText

    implicitHeight: btn.compact ? Style.controlHeightCompact : Style.controlHeight
    implicitWidth: btn.isIconOnly
                   ? implicitHeight
                   : Math.max(btn.minWidth > 0 ? btn.minWidth : Style.buttonMinWidth,
                              btnRow.implicitWidth + Style.padModal)
    Layout.preferredHeight: implicitHeight
    Layout.preferredWidth: implicitWidth
    radius: Math.min(Style.radiusMd, theme.radius)

    color: btn.disabled ? theme.surfaceInput
         : btn.isPrimary ? theme.accent
         : btn.isDefault ? (btn.isHovered ? theme.surfaceRaised : theme.surfaceInput)
         : btn.tone === "danger" ? theme.dangerSurface
         : (btn.isHovered ? theme.surfaceSelected : theme.accentSurface)

    border.width: btn.activeFocus || btn.disabled || !btn.isPrimary ? Style.hairline : 0
    border.color: btn.activeFocus ? theme.borderFocus
                : btn.disabled ? theme.borderSubtle
                : btn.isPrimary ? "transparent"
                : btn.isDefault ? (btn.isHovered ? theme.borderStrong : theme.borderSubtle)
                : btn.tone === "danger" ? (btn.isHovered ? theme.danger : theme.dangerBorder)
                : (btn.isHovered ? theme.accent : theme.accentBorder)

    RowLayout {
        id: btnRow
        anchors.centerIn: parent
        spacing: Style.gapMd

        SlidersIcon {
            visible: btn.slidersIcon
            color: btn.contentColor
            Layout.alignment: Qt.AlignVCenter
        }

        Text {
            visible: btn.label.length > 0
            text: btn.label
            color: btn.contentColor
            font.family: Style.fontSans
            font.pixelSize: Style.fontBody
            font.weight: btn.isPrimary ? Font.Medium : Font.Normal
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
