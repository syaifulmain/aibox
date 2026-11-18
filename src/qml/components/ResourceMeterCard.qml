import QtQuick
import QtQuick.Layouts
import QtQuick.Controls

// Compact resource card built on Card with a primary value, optional usage
// meter and alert colouring. Disk, memory and CPU all use the same layout.
Card {
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

    showHeader: false
    fillColor: cardHover.hovered && root.interactive ? theme.surfaceRaised : theme.surfaceInput
    borderColor: root.alertLevel === 2
                 ? theme.danger
                 : (root.alertLevel === 1
                    ? theme.warning
                    : (cardHover.hovered && root.interactive ? theme.borderStrong : theme.borderSubtle))
    cardPadding: Style.padCard
    contentSpacing: Style.gapSm
    Layout.preferredHeight: root.expanded
                          ? contentImplicitHeight + Style.padCard * 2
                          : Style.buttonMinWidth

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

    RowLayout {
        Layout.fillWidth: true
        spacing: Style.gapSm

        Text {
            Layout.fillWidth: true
            elide: Text.ElideRight
            text: root.title
            color: theme.textSecondary
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
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
            font.pixelSize: Style.fontCaption
            font.weight: Font.Medium
        }
    }

    Text {
        Layout.fillWidth: true
        elide: Text.ElideRight
        text: root.value
        color: theme.textPrimary
        font.family: Style.fontMono
        font.pixelSize: Style.fontBody
    }

    Item { Layout.fillHeight: true }

    MeterBar {
        visible: root.hasBar
        Layout.fillWidth: true
        value: root.percent
        minFillWidth: Style.gapSm
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
        Layout.topMargin: Style.gapSm
    }
}
