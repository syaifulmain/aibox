import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import "../components"

ModalDialog {
    id: changeReviewDialogRoot
    property QtObject app: null
    open: app.changeReviewVisible
    cardWidth: Math.min(parent.width - Style.modalGutter, 1020)
    cardHeight: Math.min(parent.height - Style.modalEdge, 680)
    onDismissed: app.changeReviewVisible = false

    ModalHeader {
        title: qsTr("Workspace Change Review — %1").arg(backend.selectedEnvironment.name || "")
        onClosed: app.changeReviewVisible = false

        ActionButton {
            label: app.diffEditExcludes ? qsTr("Hide Excludes") : qsTr("Excludes")
            onClicked: app.diffEditExcludes = !app.diffEditExcludes
        }
        ActionButton {
            label: qsTr("Refresh")
            disabled: backend.diffLoading || app.diffSnapName.length === 0
            onClicked: backend.loadWorkspaceDiff(backend.selectedEnvironment.name || "", app.diffSnapName)
        }
    }

    // Snapshot selector strip
    RowLayout {
        Layout.fillWidth: true
        spacing: Style.space8

        Text {
            text: qsTr("Snapshot:")
            color: theme.textMuted
            font.family: Style.fontSans
            font.pixelSize: Style.font12
        }

        ScrollView {
            Layout.fillWidth: true
            Layout.preferredHeight: 36
            clip: true

            RowLayout {
                spacing: Style.space6
                Repeater {
                    model: backend.selectedEnvironment.workspaceSnapshots || []
                    delegate: ActionButton {
                        required property string modelData
                        implicitHeight: Style.controlHeightSm
                        label: modelData
                        variant: app.diffSnapName === modelData ? "accent" : ""
                        onClicked: {
                            app.diffSnapName = modelData
                            app.diffSelectedPath = ""
                            app.diffSelectedStatus = ""
                            app.diffSelectedType = ""
                            app.diffActionError = ""
                            backend.loadWorkspaceDiff(backend.selectedEnvironment.name || "", modelData)
                        }
                    }
                }
            }
        }
    }

    // Optional exclude-pattern editor
    RowLayout {
        Layout.fillWidth: true
        visible: app.diffEditExcludes
        spacing: Style.space10

        FlatInput {
            id: diffExcludesInput
            Layout.fillWidth: true
            text: app.diffExcludesDraft
            placeholder: qsTr("Comma-separated excludes (e.g. .git/, node_modules/, build/)")
            onTextChanged: app.diffExcludesDraft = text
        }

        ActionButton {
            label: qsTr("Save Excludes")
            variant: "accent"
            onClicked: {
                const err = backend.saveDiffExcludes(diffExcludesInput.text)
                if (err.length > 0) {
                    app.diffActionError = err
                } else {
                    app.diffActionError = ""
                    app.diffEditExcludes = false
                    if (app.diffSnapName.length > 0)
                        backend.loadWorkspaceDiff(backend.selectedEnvironment.name || "", app.diffSnapName)
                }
            }
        }
    }

    // Summary bar
    RowLayout {
        Layout.fillWidth: true
        spacing: Style.space12

        Text {
            visible: app.diffSnapName.length === 0
            text: qsTr("No workspace Btrfs snapshots exist for this environment yet.")
            color: theme.textMuted
            font.family: Style.fontSans
            font.pixelSize: Style.font12
        }
        Text {
            visible: app.diffSnapName.length > 0 && backend.diffLoading
            text: qsTr("Scanning workspace changes against '%1'...").arg(app.diffSnapName)
            color: theme.accentText
            font.family: Style.fontSans
            font.pixelSize: Style.font12
        }
        Text {
            visible: app.diffSnapName.length > 0 && !backend.diffLoading && !(backend.diffSummary.error && backend.diffSummary.error.length > 0)
            text: qsTr("+%1 Added   ~%2 Modified   -%3 Deleted   (%4 total)")
                  .arg(backend.diffSummary.addedCount || 0)
                  .arg(backend.diffSummary.modifiedCount || 0)
                  .arg(backend.diffSummary.deletedCount || 0)
                  .arg(backend.diffSummary.totalCount || 0)
            color: theme.textPrimary
            font.family: Style.fontMono
            font.pixelSize: Style.font12
        }

        TagBadge {
            visible: backend.diffSummary.truncated || false
            text: qsTr("RESULTS TRUNCATED (5000 MAX)")
            tone: "warning"
            fillColor: theme.surfaceInput
            badgeHeight: Style.badgeHeightMd
            badgeRadius: Style.radius2
            textSize: Style.font10
            horizontalPadding: Style.space12
        }

        Item { Layout.fillWidth: true }
    }

    Text {
        Layout.fillWidth: true
        visible: (backend.diffSummary.error && backend.diffSummary.error.length > 0) || app.diffActionError.length > 0
        text: app.diffActionError.length > 0 ? app.diffActionError : (backend.diffSummary.error || "")
        color: theme.dangerText
        font.family: Style.fontSans
        font.pixelSize: Style.font12
        wrapMode: Text.WordWrap
    }

    // Split view: file list (left) + unified diff (right)
    RowLayout {
        Layout.fillWidth: true
        Layout.fillHeight: true
        spacing: Style.space12

        // Left pane: changed files
        Rectangle {
            Layout.preferredWidth: 370
            Layout.fillHeight: true
            radius: Math.min(Style.radius4, theme.radius)
            color: theme.surfaceInput
            border.width: Style.hairline
            border.color: theme.borderSubtle

            Text {
                anchors.centerIn: parent
                visible: !backend.diffLoading && backend.diffEntries.length === 0 && app.diffSnapName.length > 0
                text: qsTr("No workspace changes since '%1'.").arg(app.diffSnapName)
                color: theme.textMuted
                font.family: Style.fontSans
                font.pixelSize: Style.font12
            }

            ListView {
                id: diffListView
                anchors.fill: parent
                anchors.margins: Style.space8
                clip: true
                spacing: Style.space4
                model: backend.diffEntries

                delegate: Rectangle {
                    required property var modelData
                    readonly property bool isSelected: app.diffSelectedPath === (modelData.path || "")
                    width: diffListView.width
                    height: 32
                    radius: Math.min(Style.radius3, theme.radius)
                    color: isSelected ? theme.surfaceSelected : "transparent"
                    border.width: isSelected ? Style.hairline : 0
                    border.color: theme.accentBorder

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: Style.space8
                        anchors.rightMargin: Style.space8
                        spacing: Style.space8

                        Text {
                            text: modelData.status === "added" ? "+" : (modelData.status === "deleted" ? "-" : "~")
                            color: modelData.status === "added" ? theme.successText : (modelData.status === "deleted" ? theme.dangerText : theme.warningText)
                            font.family: Style.fontMono
                            font.pixelSize: Style.font12
                            font.weight: Font.Bold
                        }

                        Text {
                            Layout.fillWidth: true
                            elide: Text.ElideLeft
                            text: modelData.path || ""
                            color: theme.textPrimary
                            font.family: Style.fontMono
                            font.pixelSize: Style.font11
                        }

                        Text {
                            text: modelData.sizeText || ""
                            color: theme.textMuted
                            font.family: Style.fontMono
                            font.pixelSize: Style.font10
                        }
                    }

                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            app.diffSelectedPath = modelData.path || ""
                            app.diffSelectedStatus = modelData.status || ""
                            app.diffSelectedType = modelData.type || ""
                            app.diffActionError = ""
                            backend.loadFileDiff(backend.selectedEnvironment.name || "", app.diffSnapName, app.diffSelectedPath, app.diffSelectedStatus)
                        }
                    }
                }
            }
        }

        // Right pane: unified diff & single-file restore
        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            radius: Math.min(Style.radius4, theme.radius)
            color: theme.surfaceInput
            border.width: Style.hairline
            border.color: theme.borderSubtle

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: Style.space10
                spacing: Style.space8

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Style.space10

                    Text {
                        Layout.fillWidth: true
                        elide: Text.ElideMiddle
                        text: backend.selectedDiffInfo.length > 0
                              ? backend.selectedDiffInfo
                              : qsTr("Select a changed file to inspect its unified diff.")
                        color: backend.selectedDiffInfo.length > 0 ? theme.textPrimary : theme.textMuted
                        font.family: Style.fontMono
                        font.pixelSize: Style.font11
                    }

                    ActionButton {
                        implicitHeight: Style.controlHeightSm
                        visible: app.diffSelectedPath.length > 0 && app.diffSelectedType !== "dir" && (app.diffSelectedStatus === "modified" || app.diffSelectedStatus === "deleted")
                        label: qsTr("Restore this file")
                        variant: "accent"
                        onClicked: {
                            const err = backend.restoreSingleFile(backend.selectedEnvironment.name || "", app.diffSnapName, app.diffSelectedPath)
                            if (err.length > 0) {
                                app.diffActionError = err
                            } else {
                                app.diffActionError = ""
                                app.diffSelectedPath = ""
                                app.diffSelectedStatus = ""
                                app.diffSelectedType = ""
                            }
                        }
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    height: Style.hairline
                    color: theme.borderSubtle
                }

                ScrollView {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true

                    TextArea {
                        readOnly: true
                        selectByMouse: true
                        wrapMode: TextEdit.NoWrap
                        text: backend.selectedDiffText.length > 0
                              ? backend.selectedDiffText
                              : (backend.selectedDiffInfo.length > 0 && app.diffSelectedPath.length > 0
                                 ? backend.selectedDiffInfo
                                 : "")
                        color: backend.selectedDiffText.length > 0 ? theme.textPrimary : theme.textMuted
                        font.family: Style.fontMono
                        font.pixelSize: Style.font11
                        background: null
                    }
                }
            }
        }
    }
}
