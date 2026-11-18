import QtQuick
import QtQuick.Layouts
import "../components"

Rectangle {
    id: statusBannerRoot
    property QtObject app: null
    readonly property string tone: backend.errorMessage.length > 0 ? "danger" : "info"
    readonly property color bannerSurface: tone === "danger" ? theme.dangerSurface
                                         : tone === "warning" ? theme.warningSurface
                                         : tone === "success" ? theme.successSurface
                                         : theme.infoSurface
    readonly property color bannerBorder: tone === "danger" ? theme.dangerBorder
                                        : tone === "warning" ? theme.warningBorder
                                        : tone === "success" ? theme.successBorder
                                        : theme.infoBorder
    readonly property color bannerText: tone === "danger" ? theme.dangerText
                                      : tone === "warning" ? theme.warningText
                                      : tone === "success" ? theme.successText
                                      : theme.infoText

    Layout.fillWidth: true
    Layout.leftMargin: Style.padSection
    Layout.rightMargin: Style.padSection
    Layout.topMargin: visible ? Style.gap : 0
    Layout.preferredHeight: visible ? Math.max(Style.controlHeight, bannerCol.implicitHeight + Style.padCard * 2) : 0
    visible: (!app.createVisible && backend.busy) || backend.errorMessage.length > 0 || (!backend.busy && backend.statusMessage.length > 0)
    radius: Math.min(Style.radiusMd, theme.radius)
    color: statusBannerRoot.bannerSurface
    border.width: visible ? Style.hairline : 0
    border.color: statusBannerRoot.bannerBorder

    ColumnLayout {
        id: bannerCol
        anchors.fill: parent
        anchors.margins: Style.padCard
        spacing: Style.gapMd

        RowLayout {
            Layout.fillWidth: true
            spacing: Style.gap

            Text {
                visible: backend.busy
                text: backend.taskProgress + "%"
                color: statusBannerRoot.bannerText
                font.family: Style.fontMono
                font.pixelSize: Style.fontBody
                font.weight: Font.Medium
            }

            Text {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: backend.errorMessage.length > 0
                      ? backend.errorMessage
                      : (backend.busy ? backend.taskStage : backend.statusMessage)
                color: statusBannerRoot.bannerText
                font.family: Style.fontSans
                font.pixelSize: Style.fontBody
            }

            ActionButton {
                visible: backend.busy
                compact: true
                label: qsTr("Cancel Task")
                variant: "danger"
                onClicked: backend.cancelTask()
            }

            ActionButton {
                visible: backend.errorMessage.length > 0
                         && backend.adminFixCommand(backend.errorMessage).length > 0
                compact: true
                label: qsTr("Copy Fix")
                variant: "accent"
                onClicked: backend.copyText(backend.adminFixCommand(backend.errorMessage),
                                            qsTr("fix command"))
            }

            ActionButton {
                visible: backend.errorMessage.length > 0
                compact: true
                label: qsTr("Dismiss")
                onClicked: backend.clearError()
            }
        }

        // Progress track while busy
        MeterBar {
            visible: backend.busy
            Layout.fillWidth: true
            value: Math.max(Style.gapSm, backend.taskProgress)
            trackColor: theme.surfaceTrack
            showAtZero: true
            animationMs: 150
        }
    }
}
