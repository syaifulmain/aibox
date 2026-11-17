import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import "../components"

ModalDialog {
    id: secretsDialogRoot
    property QtObject app: null
    open: app.secretsVisible
    zOrder: 27
    cardWidth: Math.min(parent.width - Style.modalGutter, 760)
    cardHeight: Math.min(parent.height - Style.modalEdge, 600)
    onDismissed: app.secretsVisible = false

    ModalHeader {
        title: qsTr("Secrets Manager")
        subtitle: backend.secretsStorePath + " (0600)"
        subtitleMono: true
        onClosed: app.secretsVisible = false
    }

    // Store permission or form error banner
    Rectangle {
        visible: backend.secretsError.length > 0 || app.secretFormError.length > 0
        Layout.fillWidth: true
        implicitHeight: secErrTxt.implicitHeight + Style.space14
        radius: Math.min(Style.radius4, theme.radius)
        color: Style.dangerSurfaceAlt
        border.width: Style.hairline
        border.color: theme.red
        Text {
            id: secErrTxt
            anchors.fill: parent
            anchors.margins: Style.space7
            text: app.secretFormError.length > 0 ? app.secretFormError : backend.secretsError
            color: theme.red
            font.family: Style.fontSans
            font.pixelSize: Style.font11
            wrapMode: Text.WordWrap
        }
    }

    Text {
        text: qsTr("Global Secrets (%1)").arg(backend.secretsList.length)
        color: theme.muted
        font.family: Style.fontSans
        font.pixelSize: Style.font11
        font.weight: Font.Medium
    }

    Text {
        visible: (backend.selectedEnvironment.name || "").length > 0
        text: qsTr("Inject into '%1' at session start (names only stored in user.aibox.secrets):").arg(backend.selectedEnvironment.name || "")
        color: theme.muted
        font.family: Style.fontSans
        font.pixelSize: Style.font11
        wrapMode: Text.WordWrap
        Layout.fillWidth: true
    }

    Text {
        visible: backend.secretsList.length === 0
        text: qsTr("No secrets configured yet. Add one below.")
        color: theme.muted
        font.family: Style.fontSans
        font.pixelSize: Style.font11
    }

    ScrollView {
        Layout.fillWidth: true
        Layout.fillHeight: true
        clip: true

        ListView {
            width: parent.width
            spacing: Style.space8
            model: backend.secretsList

            delegate: Rectangle {
                required property var modelData
                readonly property bool revealed: (app.secretRevealMap || {})[modelData.name] === true
                readonly property bool assigned: (backend.selectedEnvironment.secretNames || []).indexOf(modelData.name) >= 0
                readonly property bool hasEnv: (backend.selectedEnvironment.name || "").length > 0
                width: ListView.view.width
                implicitHeight: 46
                radius: Math.min(Style.radius4, theme.radius)
                color: Style.surfaceInput
                border.width: Style.hairline
                border.color: Style.borderSubtle

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: Style.space12
                    anchors.rightMargin: Style.space10
                    spacing: Style.space10

                    // Toggle whether this secret is injected into the selected environment
                    Rectangle {
                        visible: hasEnv
                        Layout.preferredWidth: 16
                        Layout.preferredHeight: 16
                        radius: Style.radius2
                        color: assigned ? theme.accent : "transparent"
                        border.width: Style.hairline
                        border.color: assigned ? theme.accent : Style.borderSubtle

                        Text {
                            anchors.centerIn: parent
                            visible: assigned
                            text: "x"
                            color: theme.accentForeground
                            font.family: Style.fontMono
                            font.pixelSize: Style.font10
                            font.weight: Font.Bold
                        }

                        MouseArea {
                            anchors.fill: parent
                            anchors.margins: -Style.space8
                            enabled: !backend.busy
                            cursorShape: backend.busy ? Qt.ForbiddenCursor : Qt.PointingHandCursor
                            onClicked: app.toggleEnvSecret(modelData.name)
                        }
                    }

                    Text {
                        text: modelData.name
                        color: theme.accent
                        font.family: Style.fontMono
                        font.pixelSize: Style.font12
                        font.weight: Font.Medium
                        Layout.preferredWidth: 195
                        elide: Text.ElideRight
                    }

                    Text {
                        text: revealed ? modelData.value : modelData.maskedValue
                        color: theme.foreground
                        font.family: Style.fontMono
                        font.pixelSize: Style.font11
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                    }

                    ActionButton {
                        label: revealed ? qsTr("Hide") : qsTr("Show")
                        minWidth: 64
                        onClicked: app.toggleSecretReveal(modelData.name)
                    }

                    ActionButton {
                        label: qsTr("Copy")
                        minWidth: 62
                        onClicked: {
                            if (!backend.copySecret(modelData.name))
                                app.secretFormError = qsTr("Could not copy secret '%1' to the clipboard.").arg(modelData.name)
                        }
                    }

                    ActionButton {
                        label: qsTr("Delete")
                        variant: "danger"
                        minWidth: 68
                        onClicked: {
                            const err = backend.deleteSecret(modelData.name)
                            if (err.length > 0)
                                app.secretFormError = err
                            else
                                app.secretFormError = ""
                        }
                    }
                }
            }
        }
    }

    Rectangle {
        Layout.fillWidth: true
        height: Style.hairline
        color: Style.borderSubtle
    }

    // Add / update secret form
    ColumnLayout {
        Layout.fillWidth: true
        spacing: Style.space8

        Text {
            text: qsTr("Add or Update Secret (values are injected only at session start via tmpfs, never written to containers or backups):")
            color: theme.muted
            font.family: Style.fontSans
            font.pixelSize: Style.font11
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Style.space10

            FlatInput {
                Layout.preferredWidth: 210
                text: app.secretNameDraft
                placeholder: "ANTHROPIC_API_KEY"
                onTextChanged: {
                    if (text !== app.secretNameDraft)
                        app.secretNameDraft = text
                }
            }

            FlatInput {
                Layout.fillWidth: true
                text: app.secretValueDraft
                echoMode: app.secretDraftShow ? TextInput.Normal : TextInput.Password
                placeholder: qsTr("Secret value")
                onTextChanged: {
                    if (text !== app.secretValueDraft)
                        app.secretValueDraft = text
                }
            }

            ActionButton {
                label: app.secretDraftShow ? qsTr("Hide") : qsTr("Show")
                minWidth: 64
                onClicked: app.secretDraftShow = !app.secretDraftShow
            }

            ActionButton {
                label: qsTr("Save Secret")
                variant: "primary"
                minWidth: 116
                disabled: app.secretNameDraft.trim().length === 0
                onClicked: {
                    const err = backend.saveSecret(app.secretNameDraft.trim(), app.secretValueDraft)
                    if (err.length > 0) {
                        app.secretFormError = err
                    } else {
                        app.secretFormError = ""
                        app.secretNameDraft = ""
                        app.secretValueDraft = ""
                        app.secretDraftShow = false
                    }
                }
            }
        }
    }
}
