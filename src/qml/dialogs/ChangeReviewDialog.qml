import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Dialogs
import QtQuick.Layouts
import "../components"

Rectangle {
    id: changeReviewDialogRoot
    property QtObject app: null
    anchors.fill: parent
    visible: app.changeReviewVisible
    color: "#c406070a"

    MouseArea {
        anchors.fill: parent
        onClicked: app.changeReviewVisible = false
    }

    Rectangle {
        anchors.centerIn: parent
        width: Math.min(parent.width - 56, 1020)
        height: Math.min(parent.height - 56, 680)
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
                app: changeReviewDialogRoot.app
                title: qsTr("Workspace Change Review — %1").arg(backend.selectedEnvironment.name || "")
                onClosed: app.changeReviewVisible = false

                ActionButton {
                    app: changeReviewDialogRoot.app
                    label: app.diffEditExcludes ? qsTr("Hide Excludes") : qsTr("Excludes")
                    onClicked: app.diffEditExcludes = !app.diffEditExcludes
                }
                ActionButton {
                    app: changeReviewDialogRoot.app
                    label: qsTr("Refresh")
                    disabled: backend.diffLoading || app.diffSnapName.length === 0
                    onClicked: backend.loadWorkspaceDiff(backend.selectedEnvironment.name || "", app.diffSnapName)
                }
            }

            // Snapshot selector strip
            RowLayout {
                Layout.fillWidth: true
                spacing: 8
                Text {
                    text: qsTr("Snapshot:")
                    color: theme.muted
                    font.family: app.sansFont
                    font.pixelSize: 12
                }
                ScrollView {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 36
                    clip: true
                    RowLayout {
                        spacing: 6
                        Repeater {
                            model: backend.selectedEnvironment.workspaceSnapshots || []
                            delegate: ActionButton {
                                app: changeReviewDialogRoot.app
                                required property string modelData
                                implicitHeight: 28
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

            // Optional diffExcludes editor
            RowLayout {
                Layout.fillWidth: true
                visible: app.diffEditExcludes
                spacing: 10
                FlatInput {
                    app: changeReviewDialogRoot.app
                    id: diffExcludesInput
                    Layout.fillWidth: true
                    implicitHeight: 34
                    text: app.diffExcludesDraft
                    placeholder: qsTr("Comma-separated excludes (e.g. .git/, node_modules/, build/)")
                    onTextChanged: app.diffExcludesDraft = text
                }
                ActionButton {
                    app: changeReviewDialogRoot.app
                    implicitHeight: 34
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
                spacing: 12
                Text {
                    visible: app.diffSnapName.length === 0
                    text: qsTr("No workspace Btrfs snapshots exist for this environment yet.")
                    color: theme.muted
                    font.family: app.sansFont
                    font.pixelSize: 12
                }
                Text {
                    visible: app.diffSnapName.length > 0 && backend.diffLoading
                    text: qsTr("Scanning workspace changes against '%1'...").arg(app.diffSnapName)
                    color: theme.accent
                    font.family: app.sansFont
                    font.pixelSize: 12
                }
                Text {
                    visible: app.diffSnapName.length > 0 && !backend.diffLoading && !(backend.diffSummary.error && backend.diffSummary.error.length > 0)
                    text: qsTr("+%1 Added   ~%2 Modified   -%3 Deleted   (%4 total)")
                          .arg(backend.diffSummary.addedCount || 0)
                          .arg(backend.diffSummary.modifiedCount || 0)
                          .arg(backend.diffSummary.deletedCount || 0)
                          .arg(backend.diffSummary.totalCount || 0)
                    color: theme.foreground
                    font.family: app.monoFont
                    font.pixelSize: 12
                }
                Rectangle {
                    visible: backend.diffSummary.truncated || false
                    height: 20
                    width: truncLabel.implicitWidth + 12
                    radius: 2
                    color: app.surfaceInput
                    border.width: 1
                    border.color: theme.yellow
                    Text {
                        id: truncLabel
                        anchors.centerIn: parent
                        text: qsTr("RESULTS TRUNCATED (5000 MAX)")
                        color: theme.yellow
                        font.family: app.monoFont
                        font.pixelSize: 10
                    }
                }
                Item { Layout.fillWidth: true }
            }

            Text {
                Layout.fillWidth: true
                visible: (backend.diffSummary.error && backend.diffSummary.error.length > 0) || app.diffActionError.length > 0
                text: app.diffActionError.length > 0 ? app.diffActionError : (backend.diffSummary.error || "")
                color: theme.red
                font.family: app.sansFont
                font.pixelSize: 12
                wrapMode: Text.WordWrap
            }

            // Split view: file list (left) + unified diff (right)
            RowLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 12

                // Left pane: changed files
                Rectangle {
                    Layout.preferredWidth: 370
                    Layout.fillHeight: true
                    radius: Math.min(4, theme.radius)
                    color: app.surfaceInput
                    border.width: 1
                    border.color: app.borderSubtle

                    Text {
                        anchors.centerIn: parent
                        visible: !backend.diffLoading && backend.diffEntries.length === 0 && app.diffSnapName.length > 0
                        text: qsTr("No workspace changes since '%1'.").arg(app.diffSnapName)
                        color: theme.muted
                        font.family: app.sansFont
                        font.pixelSize: 12
                    }

                    ListView {
                        id: diffListView
                        anchors.fill: parent
                        anchors.margins: 8
                        clip: true
                        spacing: 4
                        model: backend.diffEntries

                        delegate: Rectangle {
                            required property var modelData
                            readonly property bool isSelected: app.diffSelectedPath === (modelData.path || "")
                            width: diffListView.width
                            height: 32
                            radius: Math.min(3, theme.radius)
                            color: isSelected ? app.surfaceCard : "transparent"
                            border.width: isSelected ? 1 : 0
                            border.color: theme.accent

                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: 8
                                anchors.rightMargin: 8
                                spacing: 8

                                Text {
                                    text: modelData.status === "added" ? "+" : (modelData.status === "deleted" ? "-" : "~")
                                    color: modelData.status === "added" ? theme.green : (modelData.status === "deleted" ? theme.red : theme.yellow)
                                    font.family: app.monoFont
                                    font.pixelSize: 12
                                    font.weight: Font.Bold
                                }

                                Text {
                                    Layout.fillWidth: true
                                    elide: Text.ElideLeft
                                    text: modelData.path || ""
                                    color: theme.foreground
                                    font.family: app.monoFont
                                    font.pixelSize: 11
                                }

                                Text {
                                    text: modelData.sizeText || ""
                                    color: theme.muted
                                    font.family: app.monoFont
                                    font.pixelSize: 10
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
                    radius: Math.min(4, theme.radius)
                    color: app.surfaceInput
                    border.width: 1
                    border.color: app.borderSubtle

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: 10
                        spacing: 8

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 10
                            Text {
                                Layout.fillWidth: true
                                elide: Text.ElideMiddle
                                text: backend.selectedDiffInfo.length > 0
                                      ? backend.selectedDiffInfo
                                      : qsTr("Select a changed file to inspect its unified diff.")
                                color: backend.selectedDiffInfo.length > 0 ? theme.foreground : theme.muted
                                font.family: app.monoFont
                                font.pixelSize: 11
                            }
                            ActionButton {
                                app: changeReviewDialogRoot.app
                                implicitHeight: 28
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
                            height: 1
                            color: app.borderSubtle
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
                                color: backend.selectedDiffText.length > 0 ? theme.foreground : theme.muted
                                font.family: app.monoFont
                                font.pixelSize: 11
                                background: null
                            }
                        }
                    }
                }
            }
        }
    }
}
