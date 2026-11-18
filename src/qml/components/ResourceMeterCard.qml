import QtQuick
import QtQuick.Layouts
import QtQuick.Controls

// Compact resource card with a primary value, optional usage meter and alert
// colouring. Disk, memory and CPU all use the same layout.
Rectangle {
    id: root
    property string title: ""
    property string value: ""
    property string badge: ""
    property int alertLevel: 0
    property real percent: 0
    property bool hasBar: false
    property bool active: false
    property bool interactive: false
    property bool expanded: false
    property string hint: ""
    property Component expandedContent: null

    signal clicked()

    // Expanded cards size to their content so the core grid can grow.
    Layout.preferredHeight: root.expanded
                          ? cardContent.implicitHeight + Style.space20
                          : 88
    radius: Math.min(Style.radius4, theme.radius)
    color: cardHover.hovered && root.interactive ? theme.surfaceRaised : theme.surfaceCard
    border.width: Style.hairline
    border.color: root.alertLevel === 2
                  ? theme.danger
                  : (root.alertLevel === 1
                     ? theme.warning
                     : (cardHover.hovered && root.interactive ? theme.borderStrong : theme.borderSubtle))

    HoverHandler {
        id: cardHover
        cursorShape: root.interactive ? Qt.PointingHandCursor : Qt.ArrowCursor
    }

    TapHandler {
        enabled: root.interactive
        onTapped: root.clicked()
    }

    ToolTip.visible: root.interactive && cardHover.hovered && root.hint.length > 0
    ToolTip.delay: 500
    ToolTip.text: root.hint

    ColumnLayout {
        id: cardContent
        anchors.fill: parent
        anchors.margins: Style.space10
        spacing: Style.space4

        RowLayout {
            Layout.fillWidth: true
            spacing: Style.space6

            Text {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: root.title
                color: theme.textSecondary
                font.family: Style.fontSans
                font.pixelSize: Style.font11
            }

            Text {
                text: root.badge
                color: root.alertLevel === 2
                       ? theme.dangerText
                       : (root.alertLevel === 1
                          ? theme.warningText
                          : (root.active
                             ? (root.percent >= 85 ? theme.dangerText : theme.accentText)
                             : theme.textMuted))
                font.family: Style.fontMono
                font.pixelSize: Style.font10
                font.weight: Font.Medium
            }
        }

        Text {
            Layout.fillWidth: true
            elide: Text.ElideRight
            text: root.value
            color: theme.textPrimary
            font.family: Style.fontMono
            font.pixelSize: Style.font13
        }

        Item { Layout.fillHeight: true }

        MeterBar {
            visible: root.hasBar
            Layout.fillWidth: true
            implicitHeight: 4
            value: root.percent
            minFillWidth: Style.space4
            fillColor: root.alertLevel === 2
                       ? theme.danger
                       : (root.alertLevel === 1
                          ? theme.warning
                          : (root.percent >= 85 ? theme.danger : theme.accent))
        }

        Loader {
            id: expandedLoader
            visible: root.expanded && root.expandedContent !== null
            active: visible
            sourceComponent: root.expandedContent
            Layout.fillWidth: true
            Layout.topMargin: Style.space6
        }
    }
}
