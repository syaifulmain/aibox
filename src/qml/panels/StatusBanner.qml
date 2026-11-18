import QtQuick
import QtQuick.Layouts
import "../components"

Rectangle {
    id: statusBannerRoot
    property QtObject app: null
    Layout.fillWidth: true
    Layout.preferredHeight: visible ? (backend.busy ? 68 : 42) : 0
    visible: (!app.createVisible && backend.busy) || backend.errorMessage.length > 0 || (!backend.busy && backend.statusMessage.length > 0)
    color: backend.errorMessage.length > 0 ? theme.dangerSurface : theme.surfaceCard

    ColumnLayout {
        anchors.fill: parent
        anchors.leftMargin: Style.space20
        anchors.rightMargin: Style.space20
        anchors.topMargin: Style.space8
        anchors.bottomMargin: Style.space8
        spacing: Style.space6

        RowLayout {
            Layout.fillWidth: true
            spacing: Style.space12

            Text {
                visible: backend.busy
                text: backend.taskProgress + "%"
                color: theme.accentText
                font.family: Style.fontMono
                font.pixelSize: Style.font12
            }

            Text {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: backend.errorMessage.length > 0
                      ? backend.errorMessage
                      : (backend.busy ? backend.taskStage : backend.statusMessage)
                color: backend.errorMessage.length > 0 ? theme.dangerText : theme.textPrimary
                font.family: Style.fontSans
                font.pixelSize: Style.font12
            }

            ActionButton {
                visible: backend.busy
                implicitHeight: Style.controlHeightXs
                label: qsTr("Cancel")
                variant: "danger"
                onClicked: backend.cancelTask()
            }

            ActionButton {
                visible: backend.errorMessage.length > 0
                         && backend.adminFixCommand(backend.errorMessage).length > 0
                implicitHeight: Style.controlHeightXs
                label: qsTr("Copy Fix")
                variant: "accent"
                onClicked: backend.copyText(backend.adminFixCommand(backend.errorMessage),
                                            qsTr("fix command"))
            }

            ActionButton {
                visible: backend.errorMessage.length > 0
                implicitHeight: Style.controlHeightXs
                label: qsTr("Dismiss")
                onClicked: backend.clearError()
            }
        }

        // Progress track while busy
        MeterBar {
            visible: backend.busy
            Layout.fillWidth: true
            implicitHeight: 4
            value: Math.max(4, backend.taskProgress)
            trackColor: theme.surfaceTrack
            showAtZero: true
            animationMs: 150
        }
    }

    Rectangle {
        anchors.bottom: parent.bottom
        width: parent.width
        height: Style.hairline
        color: backend.errorMessage.length > 0 ? theme.dangerBorder : theme.borderSubtle
    }
}
