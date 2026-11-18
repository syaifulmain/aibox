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
    property int textSize: Style.fontCaption
    property int textWeight: Font.Normal
    property int horizontalPadding: Style.gapMd
    property int badgeHeight: Style.badgeHeight
    property int badgeRadius: Style.radiusSm
    signal clicked()

    property color fillColor: root.tone === "accent" ? theme.surfaceSelected
                             : root.tone === "quiet" || root.tone === "accentQuiet" ? theme.surfaceInput
                             : root.tone === "success" ? theme.successSurface
                             : root.tone === "warning" ? theme.warningSurface
                             : root.tone === "danger" ? theme.dangerSurface
                             : theme.surfaceInput
    property color borderColor: root.tone === "accent" ? theme.accentBorder
                              : root.tone === "success" ? theme.successBorder
                              : root.tone === "warning" ? theme.warningBorder
                              : root.tone === "danger" ? theme.dangerBorder
                              : theme.borderSubtle
    property color textColor: root.tone === "accent" || root.tone === "accentQuiet" ? theme.accentText
                            : root.tone === "success" ? theme.successText
                            : root.tone === "warning" ? theme.warningText
                            : root.tone === "danger" ? theme.dangerText
                            : theme.textMuted

    height: badgeHeight
    width: dotVisible ? dotRow.implicitWidth + horizontalPadding
                      : plainLabel.implicitWidth + horizontalPadding
    radius: Math.min(badgeRadius, theme.radius)
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
        spacing: Style.gapSm

        Rectangle {
            width: Style.gapSm + Style.gapXs
            height: Style.gapSm + Style.gapXs
            radius: Style.radiusSm
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
