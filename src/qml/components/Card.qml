import QtQuick
import QtQuick.Layouts

// Standard surface card with optional title, subtitle and right-aligned actions.
Rectangle {
    id: root
    property string title: ""
    property string subtitle: ""
    property bool showHeader: true
    property color fillColor: theme.surfaceInput
    property color borderColor: theme.borderSubtle
    property int cardPadding: Style.padCard
    property int contentSpacing: Style.gap
    property alias actions: actionsRow.data
    default property alias contentChildren: bodyCol.data
    readonly property real contentImplicitHeight: cardCol.implicitHeight

    Layout.fillWidth: true
    implicitHeight: cardCol.implicitHeight + root.cardPadding * 2
    radius: Math.min(Style.radiusMd, theme.radius)
    color: root.fillColor
    border.width: Style.hairline
    border.color: root.borderColor

    ColumnLayout {
        id: cardCol
        anchors.fill: parent
        anchors.margins: root.cardPadding
        spacing: root.contentSpacing

        RowLayout {
            id: headerRow
            Layout.fillWidth: true
            spacing: Style.gap
            visible: root.showHeader && (root.title.length > 0 || root.subtitle.length > 0 || actionsRow.children.length > 0)

            ColumnLayout {
                Layout.fillWidth: true
                Layout.minimumWidth: 0
                spacing: Style.gapXs

                Text {
                    visible: root.title.length > 0
                    Layout.fillWidth: true
                    elide: Text.ElideRight
                    text: root.title
                    color: theme.textPrimary
                    font.family: Style.fontSans
                    font.pixelSize: Style.fontTitle
                    font.weight: Font.Medium
                }

                Text {
                    visible: root.subtitle.length > 0
                    Layout.fillWidth: true
                    elide: Text.ElideRight
                    text: root.subtitle
                    color: theme.textMuted
                    font.family: Style.fontSans
                    font.pixelSize: Style.fontSmall
                }
            }

            RowLayout {
                id: actionsRow
                spacing: Style.gap
                Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
            }
        }

        ColumnLayout {
            id: bodyCol
            Layout.fillWidth: true
            spacing: root.contentSpacing
        }
    }
}
