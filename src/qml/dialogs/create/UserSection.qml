import QtQuick
import QtQuick.Layouts
import "../../components"

// Optional non-root default user for fresh environments.
ColumnLayout {
    id: root
    property QtObject app: null

    visible: !root.app.createFromTemplate
    Layout.fillWidth: true
    spacing: Style.gapSm

    Text {
        text: qsTr("Default User")
        color: theme.textPrimary
        font.family: Style.fontSans
        font.pixelSize: Style.fontBody
        font.weight: Font.Medium
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: Style.gapMd

        SelectCard {
            Layout.fillWidth: true
            selected: !root.app.newEnvCreateUser
            title: qsTr("Root only")
            subtitle: qsTr("Sessions run as root (current behaviour).")
            mediumWhenSelected: true
            onClicked: root.app.newEnvCreateUser = false
        }

        SelectCard {
            Layout.fillWidth: true
            selected: root.app.newEnvCreateUser
            title: qsTr("Create non-root user")
            subtitle: qsTr("Sessions log in as a regular user.")
            mediumWhenSelected: true
            onClicked: root.app.newEnvCreateUser = true
        }
    }

    ColumnLayout {
        visible: root.app.newEnvCreateUser
        Layout.fillWidth: true
        spacing: Style.gapSm

        FlatInput {
            id: loginUserField
            Layout.fillWidth: true
            placeholder: qsTr("e.g. dev")
            text: root.app.newEnvUserName
            onTextChanged: root.app.newEnvUserName = text
        }

        CheckRow {
            checked: root.app.newEnvUserSudo
            text: qsTr("Allow passwordless sudo")
            onToggled: root.app.newEnvUserSudo = !root.app.newEnvUserSudo
        }

        Text {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: qsTr("Terminal, aibox enter and aibox exec log in as this user. The UID matches your host user when possible so workspace files keep your ownership.")
            color: theme.textMuted
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
        }
    }
}
