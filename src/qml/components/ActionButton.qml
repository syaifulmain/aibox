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
    readonly property color toneColor: btn.isDefault ? theme.foreground
                                    : btn.isPrimary ? theme.accent
                                    : btn.tone === "danger" ? theme.red
                                    : btn.tone === "success" ? theme.green
                                    : theme.accent
    readonly property color contentColor: btn.isPrimary ? theme.accentForeground : btn.toneColor

    // Tinted hover surface used by every non-primary, non-default tone.
    function tint(color, alpha) {
        return Qt.rgba(color.r, color.g, color.b, alpha)
    }

    implicitHeight: Style.controlHeight
    implicitWidth: minWidth > 0 ? Math.max(minWidth, btnRow.implicitWidth + Style.space24)
                                : (label === "?" ? Style.controlHeight : btnRow.implicitWidth + Style.space24)
    Layout.preferredHeight: implicitHeight
    Layout.preferredWidth: implicitWidth
    radius: Math.min(Style.radius4, theme.radius)
    opacity: btn.disabled ? Style.opacityDisabled : (btnMouse.containsMouse ? Style.opacityHover : 1.0)

    color: btn.isPrimary ? theme.accent
         : btn.isDefault ? (btnMouse.containsMouse ? theme.lighterBackground : Style.surfaceCard)
         : btn.tint(btn.toneColor, btnMouse.containsMouse ? 0.22 : 0.12)

    border.width: btn.isPrimary ? 0 : Style.hairline
    border.color: btn.isPrimary ? "transparent"
                : btn.isDefault ? (btnMouse.containsMouse ? theme.muted : Style.borderSubtle)
                : btn.toneColor

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
