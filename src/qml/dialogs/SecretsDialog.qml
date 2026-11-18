import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import "../components"

// Global secret manager, also reused in environment scope. In environment
// scope global secrets are read-only and instance-only secrets can be added.
ModalDialog {
    id: secretsDialogRoot
    property QtObject app: null
    readonly property bool envScope: app.secretsScope === "environment"
    readonly property string envName: app.secretsEnvName || ""
    readonly property int rowCount: envScope ? backend.environmentSecrets.length
                                             : backend.secretsList.length

    open: app.secretsVisible
    zOrder: 27
    size: "md"
    cardHeight: Math.min(parent.height - Style.modalGutter,
                         260 + Math.min(rowCount, 7) * 52 + (envScope ? 40 : 0))
    onDismissed: app.secretsVisible = false

    function saveDraft() {
        const err = secretsDialogRoot.envScope
                    ? backend.saveEnvironmentSecret(secretsDialogRoot.envName,
                                                    app.secretNameDraft.trim(), app.secretValueDraft)
                    : backend.saveSecret(app.secretNameDraft.trim(), app.secretValueDraft)
        if (err.length > 0) {
            app.secretFormError = err
        } else {
            app.secretFormError = ""
            app.secretNameDraft = ""
            app.secretValueDraft = ""
        }
    }

    function deleteRow(row) {
        const err = secretsDialogRoot.envScope
                    ? backend.deleteEnvironmentSecret(secretsDialogRoot.envName, row.name)
                    : backend.deleteSecret(row.name)
        app.secretFormError = err.length > 0 ? err : ""
    }

    function copyRow(row) {
        const ok = secretsDialogRoot.envScope
                   ? backend.copyEnvironmentSecret(secretsDialogRoot.envName, row.name)
                   : backend.copySecret(row.name)
        if (!ok)
            app.secretFormError = qsTr("Could not copy secret '%1' to the clipboard.").arg(row.name)
    }

    ModalHeader {
        title: secretsDialogRoot.envScope ? qsTr("Secrets · %1").arg(secretsDialogRoot.envName)
                                          : qsTr("Secrets Manager")
        subtitle: secretsDialogRoot.envScope
                  ? backend.environmentSecretsStorePath(secretsDialogRoot.envName) + " (0600)"
                  : backend.secretsStorePath + " (0600)"
        subtitleMono: true
        onClosed: app.secretsVisible = false
    }

    // Store permission or form error banner
    Rectangle {
        visible: app.secretFormError.length > 0
                 || (secretsDialogRoot.envScope ? backend.environmentSecretsError.length > 0
                                                : backend.secretsError.length > 0)
        Layout.fillWidth: true
        implicitHeight: secErrTxt.implicitHeight + Style.padSection
        radius: Math.min(Style.radiusMd, theme.radius)
        color: theme.dangerSurface
        border.width: Style.hairline
        border.color: theme.dangerBorder
        Text {
            id: secErrTxt
            anchors.fill: parent
            anchors.margins: Style.gapMd
            text: app.secretFormError.length > 0
                  ? app.secretFormError
                  : (secretsDialogRoot.envScope ? backend.environmentSecretsError
                                                : backend.secretsError)
            color: theme.dangerText
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
            wrapMode: Text.WordWrap
        }
    }

    Text {
        text: secretsDialogRoot.envScope
              ? qsTr("Secrets for '%1' (%2)").arg(secretsDialogRoot.envName)
                    .arg(backend.environmentSecrets.length)
              : qsTr("Global Secrets (%1)").arg(backend.secretsList.length)
        color: theme.textMuted
        font.family: Style.fontSans
        font.pixelSize: Style.fontSmall
        font.weight: Font.Medium
    }

    Text {
        visible: secretsDialogRoot.envScope && backend.secretsList.length > 0
        text: qsTr("Global secrets are available to every environment. Their checkbox controls injection here; delete them in Tools → Secrets.")
        color: theme.textMuted
        font.family: Style.fontSans
        font.pixelSize: Style.fontSmall
        wrapMode: Text.WordWrap
        Layout.fillWidth: true
    }

    Text {
        visible: !secretsDialogRoot.envScope
        text: qsTr("Checked secrets appear in every environment's Secrets dialog. Each environment controls its own injection checkbox there.")
        color: theme.textMuted
        font.family: Style.fontSans
        font.pixelSize: Style.fontSmall
        wrapMode: Text.WordWrap
        Layout.fillWidth: true
    }

    Text {
        visible: secretsDialogRoot.envScope ? backend.environmentSecrets.length === 0
                                            : backend.secretsList.length === 0
        text: secretsDialogRoot.envScope
              ? qsTr("No secrets for this environment yet. Add one below.")
              : qsTr("No secrets configured yet. Add one below.")
        color: theme.textMuted
        font.family: Style.fontSans
        font.pixelSize: Style.fontSmall
    }

    ScrollView {
        Layout.fillWidth: true
        Layout.fillHeight: true
        clip: true
        ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
        ScrollBar.vertical.policy: ScrollBar.AlwaysOff

        ListView {
            width: parent.width
            spacing: Style.gapMd
            section.property: secretsDialogRoot.envScope ? "scope" : ""
            section.delegate: Item {
                width: ListView.view.width
                height: Style.controlHeightCompact

                Text {
                    anchors.left: parent.left
                    anchors.verticalCenter: parent.verticalCenter
                    text: section === "environment" ? qsTr("Only this environment")
                                                    : qsTr("Global · managed in Tools")
                    color: theme.textMuted
                    font.family: Style.fontSans
                    font.pixelSize: Style.fontCaption
                    font.weight: Font.Medium
                }
            }
            model: secretsDialogRoot.envScope ? backend.environmentSecrets : backend.secretsList

            delegate: Rectangle {
                required property var modelData
                readonly property bool isGlobal: modelData.scope !== "environment"
                readonly property bool assigned: secretsDialogRoot.envScope
                                                 ? modelData.assigned === true
                                                 : modelData.visibleInEnvironments === true
                readonly property bool canDelete: !secretsDialogRoot.envScope || !isGlobal
                width: ListView.view.width
                implicitHeight: secretRow.implicitHeight + Style.padCard * 2
                radius: Math.min(Style.radiusMd, theme.radius)
                color: theme.surfaceInput
                border.width: Style.hairline
                border.color: theme.borderSubtle

                RowLayout {
                    id: secretRow
                    anchors.fill: parent
                    anchors.leftMargin: Style.padCard
                    anchors.rightMargin: Style.gap
                    spacing: Style.gap

                    // Global manager: show or hide in environment dialogs.
                    // Environment scope: inject into this environment.
                    Rectangle {
                        Layout.preferredWidth: Style.padSection
                        Layout.preferredHeight: Style.padSection
                        radius: Math.min(Style.radiusSm, theme.radius)
                        color: assigned ? theme.accent : "transparent"
                        border.width: Style.hairline
                        border.color: assigned ? theme.accent : theme.borderSubtle

                        Text {
                            anchors.centerIn: parent
                            visible: assigned
                            text: "x"
                            color: theme.onAccent
                            font.family: Style.fontMono
                            font.pixelSize: Style.fontCaption
                            font.weight: Font.Bold
                        }

                        MouseArea {
                            anchors.fill: parent
                            anchors.margins: -Style.gapMd
                            enabled: !backend.busy
                            cursorShape: backend.busy ? Qt.ForbiddenCursor : Qt.PointingHandCursor
                            onClicked: {
                                if (!secretsDialogRoot.envScope)
                                    app.toggleGlobalSecretVisibility(modelData.name)
                                else if (isGlobal)
                                    app.toggleEnvSecret(modelData.name)
                                else
                                    app.toggleLocalSecret(modelData.name)
                            }
                        }
                    }

                    Text {
                        text: modelData.name
                        color: theme.accentText
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontBody
                        font.weight: Font.Medium
                        Layout.preferredWidth: 200
                        elide: Text.ElideRight
                    }

                    Text {
                        text: modelData.maskedValue
                        color: theme.textPrimary
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontSmall
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                    }

                    ActionButton {
                        compact: true
                        label: qsTr("Copy")
                        onClicked: secretsDialogRoot.copyRow(modelData)
                    }

                    ActionButton {
                        compact: true
                        visible: canDelete
                        label: qsTr("Delete")
                        variant: "danger"
                        onClicked: secretsDialogRoot.deleteRow(modelData)
                    }
                }
            }
        }
    }

    Rectangle {
        Layout.fillWidth: true
        height: Style.hairline
        color: theme.borderSubtle
    }

    // Add / update secret form
    ColumnLayout {
        Layout.fillWidth: true
        spacing: Style.gapMd

        Text {
            text: secretsDialogRoot.envScope
                  ? qsTr("Add a secret that only '%1' receives. It is injected automatically at session start:").arg(secretsDialogRoot.envName)
                  : qsTr("Add or Update Secret (values are injected only at session start via tmpfs, never written to containers or backups):")
            color: theme.textMuted
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Style.gap

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
                echoMode: TextInput.Password
                placeholder: qsTr("Secret value")
                onTextChanged: {
                    if (text !== app.secretValueDraft)
                        app.secretValueDraft = text
                }
            }

            ActionButton {
                label: qsTr("Save")
                variant: "primary"
                disabled: app.secretNameDraft.trim().length === 0
                onClicked: secretsDialogRoot.saveDraft()
            }
        }
    }
}
