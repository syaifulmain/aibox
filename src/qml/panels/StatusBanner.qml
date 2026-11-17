import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Dialogs
import QtQuick.Layouts
import "../components"

Rectangle {
    property QtObject app: null
    id: statusBannerRoot
    Layout.fillWidth: true
    Layout.preferredHeight: visible ? (backend.busy ? 68 : 42) : 0
    visible: (!app.createVisible && backend.busy) || backend.errorMessage.length > 0 || (!backend.busy && backend.statusMessage.length > 0)
    color: backend.errorMessage.length > 0 ? "#241217" : app.surfaceCard

    ColumnLayout {
        anchors.fill: parent
        anchors.leftMargin: 20
        anchors.rightMargin: 20
        anchors.topMargin: 8
        anchors.bottomMargin: 8
        spacing: 6

        RowLayout {
            Layout.fillWidth: true
            spacing: 12

            Text {
                visible: backend.busy
                text: backend.taskProgress + "%"
                color: theme.accent
                font.family: app.monoFont
                font.pixelSize: 12
            }

            Text {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: backend.errorMessage.length > 0
                      ? backend.errorMessage
                      : (backend.busy ? backend.taskStage : backend.statusMessage)
                color: backend.errorMessage.length > 0 ? theme.red : theme.foreground
                font.family: app.sansFont
                font.pixelSize: 12
            }

            Text {
                visible: backend.busy && backend.taskLog.length > 0
                Layout.maximumWidth: 340
                elide: Text.ElideLeft
                text: backend.taskLog
                color: theme.muted
                font.family: app.monoFont
                font.pixelSize: 11
            }

            ActionButton {
                app: statusBannerRoot.app
                visible: backend.busy
                implicitHeight: 26
                label: qsTr("Cancel")
                danger: true
                onClicked: backend.cancelTask()
            }

            ActionButton {
                app: statusBannerRoot.app
                visible: backend.errorMessage.length > 0
                         && backend.adminFixCommand(backend.errorMessage).length > 0
                implicitHeight: 26
                label: qsTr("Copy Fix")
                accent: true
                onClicked: backend.copyText(backend.adminFixCommand(backend.errorMessage),
                                            qsTr("fix command"))
            }

            ActionButton {
                app: statusBannerRoot.app
                visible: backend.errorMessage.length > 0
                implicitHeight: 26
                label: qsTr("Dismiss")
                onClicked: backend.clearError()
            }
        }

        // Progress track when busy
        Rectangle {
            visible: backend.busy
            Layout.fillWidth: true
            height: 4
            radius: 2
            color: app.surfaceInput

            Rectangle {
                height: parent.height
                width: parent.width * (Math.max(4, backend.taskProgress) / 100.0)
                radius: 2
                color: theme.accent
                Behavior on width { NumberAnimation { duration: 150 } }
            }
        }
    }

    Rectangle {
        anchors.bottom: parent.bottom
        width: parent.width
        height: 1
        color: backend.errorMessage.length > 0 ? theme.red : app.borderSubtle
    }
}
