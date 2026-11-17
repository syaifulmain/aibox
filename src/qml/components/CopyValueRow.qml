import QtQuick
import QtQuick.Layouts

// Label/value row with an optional copy-to-clipboard affordance.
RowLayout {
    id: root
    property string label: ""
    property string value: ""
    property string tone: "normal" // normal | up | down | warn
    property string copyValue: ""
    property bool justCopied: false
    readonly property bool copyable: copyValue.length > 0

    Layout.fillWidth: true
    Layout.minimumWidth: 0
    spacing: Style.space10

    Text {
        text: root.label
        color: root.copyable && hoverArea.hovered ? theme.foreground : theme.muted
        font.family: Style.fontSans
        font.pixelSize: Style.font11
    }

    Item { Layout.fillWidth: true }

    Text {
        Layout.minimumWidth: 0
        elide: Text.ElideRight
        text: root.justCopied ? qsTr("Copied") : root.value
        color: root.justCopied
               ? theme.accent
               : (root.value === "—"
                  ? theme.muted
                  : (root.tone === "down"
                     ? theme.green
                     : (root.tone === "up"
                        ? theme.accent
                        : (root.tone === "warn" ? Style.warning : theme.foreground))))
        font.family: Style.fontMono
        font.pixelSize: Style.font12
    }

    // Monochrome copy glyph: hover hint for copyable values
    Item {
        visible: root.copyable
        Layout.alignment: Qt.AlignVCenter
        Layout.preferredWidth: 10
        Layout.preferredHeight: 10
        opacity: root.justCopied ? 1.0 : (hoverArea.hovered ? 0.95 : Style.opacityDisabled)

        Rectangle {
            x: 0
            y: 3
            width: 7
            height: 7
            radius: 1
            color: "transparent"
            border.width: Style.hairline
            border.color: root.justCopied ? theme.accent : (hoverArea.hovered ? theme.accent : theme.muted)
        }
        Rectangle {
            x: 3
            y: 0
            width: 7
            height: 7
            radius: 1
            color: Style.surfaceInput
            border.width: Style.hairline
            border.color: root.justCopied ? theme.accent : (hoverArea.hovered ? theme.accent : theme.muted)
        }
    }

    HoverHandler {
        id: hoverArea
        enabled: root.copyable
        cursorShape: Qt.PointingHandCursor
    }

    TapHandler {
        enabled: root.copyable
        onTapped: {
            if (backend.copyText(root.copyValue, root.label)) {
                root.justCopied = true
                copyResetTimer.restart()
            }
        }
    }

    Timer {
        id: copyResetTimer
        interval: 1200
        onTriggered: root.justCopied = false
    }
}
