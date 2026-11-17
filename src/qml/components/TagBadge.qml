import QtQuick
import QtQuick.Layouts

// Small status chip used for states, counts and tags.
//
// The tone selects a default colour triple; a caller can override any of the
// three colours when a screen needs a different combination.
Rectangle {
    id: root
    property string text: ""
    property string tone: "neutral" // neutral | quiet | accent | accentQuiet | success | warning | danger
    property bool dotVisible: false
    property color dotColor: root.textColor
    property bool interactive: false
    property int textSize: Style.font9
    property int textWeight: Font.Normal
    property int horizontalPadding: Style.space10
    property int badgeHeight: Style.badgeHeight
    property int badgeRadius: Style.radius2
    signal clicked()

    property color fillColor: root.tone === "accent" ? Style.surfaceSelected
                             : root.tone === "quiet" || root.tone === "accentQuiet" ? Style.surfaceInput
                             : root.tone === "success" ? Style.successSurface
                             : root.tone === "warning" ? Style.warningSurfaceAlt
                             : root.tone === "danger" ? Style.dangerTint
                             : Style.surfaceCard
    property color borderColor: root.tone === "accent" ? theme.accent
                              : root.tone === "success" ? Style.successBorder
                              : root.tone === "warning" ? Style.warning
                              : root.tone === "danger" ? theme.red
                              : Style.borderSubtle
    property color textColor: root.tone === "accent" || root.tone === "accentQuiet" ? theme.accent
                            : root.tone === "success" ? theme.green
                            : root.tone === "warning" ? Style.warning
                            : root.tone === "danger" ? theme.red
                            : theme.muted

    height: badgeHeight
    width: dotVisible ? dotRow.implicitWidth + horizontalPadding
                      : plainLabel.implicitWidth + horizontalPadding
    radius: badgeRadius
    color: root.fillColor
    border.width: Style.hairline
    border.color: root.borderColor

    // Text-only badges keep the plain centered label used across the app.
    Text {
        id: plainLabel
        visible: !root.dotVisible
        anchors.centerIn: parent
        text: root.text
        color: root.textColor
        font.family: Style.fontMono
        font.pixelSize: root.textSize
        font.weight: root.textWeight
    }

    // Status badges add a small state dot before the label.
    RowLayout {
        id: dotRow
        visible: root.dotVisible
        anchors.centerIn: parent
        spacing: Style.space6

        Rectangle {
            width: 6
            height: 6
            radius: 3
            color: root.dotColor
            Layout.alignment: Qt.AlignVCenter
        }

        Text {
            text: root.text
            Layout.alignment: Qt.AlignVCenter
            color: root.textColor
            font.family: Style.fontMono
            font.pixelSize: root.textSize
            font.weight: root.textWeight
        }
    }

    MouseArea {
        anchors.fill: parent
        enabled: root.interactive
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
