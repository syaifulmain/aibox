import QtQuick
import QtQuick.Layouts

// Flat action button with an optional sliders icon.
Rectangle {
    id: btn
    property string label: ""
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

    readonly property string tone: btn.primary || btn.variant === "primary" ? "primary"
                                 : (btn.danger || btn.variant === "danger") ? "danger"
                                 : (btn.accent || btn.variant === "accent" || btn.variant === "theme") ? "accent"
                                 : (btn.success || btn.variant === "success") ? "success"
                                 : "default"
    readonly property bool isPrimary: tone === "primary"
    readonly property bool isDefault: tone === "default"
    readonly property bool isHovered: btnMouse.containsMouse && !btn.disabled
    readonly property color contentColor: btn.disabled ? theme.textDisabled
                                      : btn.isPrimary ? theme.textOnAccent
                                      : btn.isDefault ? theme.textPrimary
                                      : btn.tone === "danger" ? theme.dangerText
                                      : btn.tone === "success" ? theme.successText
                                      : theme.accentText

    implicitHeight: Style.controlHeight
    implicitWidth: minWidth > 0 ? Math.max(minWidth, btnRow.implicitWidth + Style.space24)
                                : (label === "?" ? Style.controlHeight : btnRow.implicitWidth + Style.space24)
    Layout.preferredHeight: implicitHeight
    Layout.preferredWidth: implicitWidth
    radius: Math.min(Style.radius4, theme.radius)
    opacity: btn.disabled ? Style.opacityDisabled : 1.0

    color: btn.isPrimary ? theme.accent
         : btn.isDefault ? (btn.isHovered ? theme.surfaceRaised : theme.surfaceCard)
         : btn.tone === "danger" ? theme.dangerSurface
         : btn.tone === "success" ? theme.successSurface
         : (btn.isHovered ? theme.surfaceSelected : theme.accentSurface)

    border.width: btn.activeFocus || !btn.isPrimary ? Style.hairline : 0
    border.color: btn.activeFocus ? theme.borderFocus
                : btn.isPrimary ? "transparent"
                : btn.isDefault ? (btn.isHovered ? theme.borderStrong : theme.borderSubtle)
                : btn.tone === "danger" ? (btn.isHovered ? theme.danger : theme.dangerBorder)
                : btn.tone === "success" ? (btn.isHovered ? theme.success : theme.successBorder)
                : (btn.isHovered ? theme.accent : theme.accentBorder)

    RowLayout {
        id: btnRow
        anchors.centerIn: parent
        spacing: Style.space8

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
            font.pixelSize: Style.font12
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
