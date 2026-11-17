import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Dialogs
import QtQuick.Layouts
import "../components"

Rectangle {
    id: secretsDialogRoot
    property QtObject app: null
    visible: app.secretsVisible
    anchors.fill: parent
    color: "#c406070a"
    z: 27

    MouseArea {
        anchors.fill: parent
        onClicked: app.secretsVisible = false
    }

    Rectangle {
        width: Math.min(760, parent.width - 56)
        height: Math.min(600, parent.height - 56)
        anchors.centerIn: parent
        radius: Math.min(8, theme.radius)
        color: app.surfaceCard
        border.width: 1
        border.color: theme.accent

        MouseArea { anchors.fill: parent }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 24
            spacing: 12

            ModalHeader {
                app: secretsDialogRoot.app
                title: qsTr("Secrets Manager")
                subtitle: backend.secretsStorePath + " (0600)"
                subtitleMono: true
                onClosed: app.secretsVisible = false
            }

            // Store permission or form error banner
            Rectangle {
                visible: backend.secretsError.length > 0 || app.secretFormError.length > 0
                Layout.fillWidth: true
                implicitHeight: secErrTxt.implicitHeight + 14
                radius: Math.min(4, theme.radius)
                color: "#291418"
                border.width: 1
                border.color: app.redColor
                Text {
                    id: secErrTxt
                    anchors.fill: parent
                    anchors.margins: 7
                    text: app.secretFormError.length > 0 ? app.secretFormError : backend.secretsError
                    color: app.redColor
                    font.family: app.sansFont
                    font.pixelSize: 11
                    wrapMode: Text.WordWrap
                }
            }

            // Global secret list
            Text {
                text: qsTr("Global Secrets (%1)").arg(backend.secretsList.length)
                color: theme.muted
                font.family: app.sansFont
                font.pixelSize: 11
                font.weight: Font.Medium
            }

            Text {
                visible: (backend.selectedEnvironment.name || "").length > 0
                text: qsTr("Inject into '%1' at session start (names only stored in user.aibox.secrets):").arg(backend.selectedEnvironment.name || "")
                color: theme.muted
                font.family: app.sansFont
                font.pixelSize: 11
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }

            Text {
                visible: backend.secretsList.length === 0
                text: qsTr("No secrets configured yet. Add one below.")
                color: theme.muted
                font.family: app.sansFont
                font.pixelSize: 11
            }

            ScrollView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true

                ListView {
                    width: parent.width
                    spacing: 8
                    model: backend.secretsList

                    delegate: Rectangle {
                        required property var modelData
                        readonly property bool revealed: (app.secretRevealMap || {})[modelData.name] === true
                        readonly property bool assigned: (backend.selectedEnvironment.secretNames || []).indexOf(modelData.name) >= 0
                        readonly property bool hasEnv: (backend.selectedEnvironment.name || "").length > 0
                        width: ListView.view.width
                        implicitHeight: 46
                        radius: Math.min(4, theme.radius)
                        color: app.surfaceInput
                        border.width: 1
                        border.color: app.borderSubtle

                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 12
                            anchors.rightMargin: 10
                            spacing: 10

                            Rectangle {
                                visible: hasEnv
                                Layout.preferredWidth: 16
                                Layout.preferredHeight: 16
                                radius: 2
                                color: assigned ? theme.accent : "transparent"
                                border.width: 1
                                border.color: assigned ? theme.accent : app.borderSubtle

                                Text {
                                    anchors.centerIn: parent
                                    visible: assigned
                                    text: "x"
                                    color: theme.accentForeground
                                    font.family: app.monoFont
                                    font.pixelSize: 10
                                    font.weight: Font.Bold
                                }

                                MouseArea {
                                    anchors.fill: parent
                                    anchors.margins: -8
                                    enabled: !backend.busy
                                    cursorShape: backend.busy ? Qt.ForbiddenCursor : Qt.PointingHandCursor
                                    onClicked: app.toggleEnvSecret(modelData.name)
                                }
                            }

                            Text {
                                text: modelData.name
                                color: theme.accent
                                font.family: app.monoFont
                                font.pixelSize: 12
                                font.weight: Font.Medium
                                Layout.preferredWidth: 195
                                elide: Text.ElideRight
                            }

                            Text {
                                text: revealed ? modelData.value : modelData.maskedValue
                                color: theme.foreground
                                font.family: app.monoFont
                                font.pixelSize: 11
                                Layout.fillWidth: true
                                elide: Text.ElideRight
                            }

                            ActionButton {
                                app: secretsDialogRoot.app
                                label: revealed ? qsTr("Hide") : qsTr("Show")
                                minWidth: 64
                                onClicked: app.toggleSecretReveal(modelData.name)
                            }

                            ActionButton {
                                app: secretsDialogRoot.app
                                label: qsTr("Copy")
                                minWidth: 62
                                onClicked: {
                                    if (!backend.copySecret(modelData.name))
                                        app.secretFormError = qsTr("Could not copy secret '%1' to the clipboard.").arg(modelData.name)
                                }
                            }

                            ActionButton {
                                app: secretsDialogRoot.app
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

            Rectangle { Layout.fillWidth: true; height: 1; color: app.borderSubtle }

            // Add / update secret form
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 8

                Text {
                    text: qsTr("Add or Update Secret (values are injected only at session start via tmpfs, never written to containers or backups):")
                    color: theme.muted
                    font.family: app.sansFont
                    font.pixelSize: 11
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 10

                    FlatInput {
                        app: secretsDialogRoot.app
                        Layout.preferredWidth: 210
                        implicitHeight: 34
                        text: app.secretNameDraft
                        placeholder: "ANTHROPIC_API_KEY"
                        onTextChanged: {
                            if (text !== app.secretNameDraft)
                                app.secretNameDraft = text
                        }
                    }

                    FlatInput {
                        app: secretsDialogRoot.app
                        Layout.fillWidth: true
                        implicitHeight: 34
                        text: app.secretValueDraft
                        echoMode: app.secretDraftShow ? TextInput.Normal : TextInput.Password
                        placeholder: qsTr("Secret value")
                        onTextChanged: {
                            if (text !== app.secretValueDraft)
                                app.secretValueDraft = text
                        }
                    }

                    ActionButton {
                        app: secretsDialogRoot.app
                        label: app.secretDraftShow ? qsTr("Hide") : qsTr("Show")
                        minWidth: 64
                        onClicked: app.secretDraftShow = !app.secretDraftShow
                    }

                    ActionButton {
                        app: secretsDialogRoot.app
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
    }
}
