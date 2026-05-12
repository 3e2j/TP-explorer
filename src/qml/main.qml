import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import ArcDiff

ApplicationWindow {
    visible: true
    width: 1000
    height: 750
    title: "ISO Diff (files/ vs folder)"

    DiffBackend {
        id: backend
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 12
        spacing: 8

        Label {
            text: "ISO file (drag and drop or click to browse)"
            font.bold: true
        }
        Rectangle {
            id: isoDropArea
            Layout.fillWidth: true
            Layout.preferredHeight: 80
            color: isoMouseArea.containsMouse ? "#e0e0ff" : "#f5f5f5"
            border.color: "#ccc"
            border.width: 2
            radius: 4

            ColumnLayout {
                anchors.centerIn: parent
                anchors.margins: 10
                width: parent.width - 20
                Label {
                    text: isoPathField.text || "Drop ISO file here or click to browse"
                    color: isoPathField.text ? "#000" : "#999"
                    Layout.fillWidth: true
                    wrapMode: Text.WrapAtWordBoundaryOrAnywhere
                    font.pixelSize: 12
                }
            }

            MouseArea {
                id: isoMouseArea
                anchors.fill: parent
                hoverEnabled: true
                onClicked: isoFileDialog.open()
            }

            DropArea {
                anchors.fill: parent
                onDropped: drop => {
                    if (drop.hasUrls) {
                        let url = drop.urls[0];
                        isoPathField.text = String(url).replace("file://", "");
                    }
                }
            }
        }
        TextField {
            id: isoPathField
            visible: false
        }

        Label {
            text: "Comparison folder (drag and drop or click to browse)"
            font.bold: true
        }
        Rectangle {
            id: folderDropArea
            Layout.fillWidth: true
            Layout.preferredHeight: 80
            color: folderMouseArea.containsMouse ? "#e0ffe0" : "#f5f5f5"
            border.color: "#ccc"
            border.width: 2
            radius: 4

            ColumnLayout {
                anchors.centerIn: parent
                anchors.margins: 10
                width: parent.width - 20
                Label {
                    text: folderPathField.text || "Drop folder here or click to browse"
                    color: folderPathField.text ? "#000" : "#999"
                    Layout.fillWidth: true
                    wrapMode: Text.WrapAtWordBoundaryOrAnywhere
                    font.pixelSize: 12
                }
            }

            MouseArea {
                id: folderMouseArea
                anchors.fill: parent
                hoverEnabled: true
                onClicked: folderFileDialog.open()
            }

            DropArea {
                anchors.fill: parent
                onDropped: drop => {
                    if (drop.hasUrls) {
                        let url = drop.urls[0];
                        folderPathField.text = String(url).replace("file://", "");
                    }
                }
            }
        }
        TextField {
            id: folderPathField
            visible: false
        }

        Button {
            id: compareButton
            Layout.fillWidth: true
            Layout.preferredHeight: 50
            text: "Compare"
            font.pixelSize: 14
            onClicked: {
                resultArea.text = "Comparing...";
                resultArea.text = backend.compare_iso_with_folder(isoPathField.text, folderPathField.text);
            }
        }

        Label {
            text: "Results"
            font.bold: true
        }
        ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            TextArea {
                id: resultArea
                wrapMode: TextArea.NoWrap
                readOnly: true
                text: "Results will appear here. Supports GameCube ISOs (raw or GZ2E compressed)."
                font.family: "monospace"
                font.pixelSize: 10
            }
        }
    }

    // Fixed: FileDialog instead of FolderDialog so users can select a .iso file
    FileDialog {
        id: isoFileDialog
        title: "Select ISO file"
        nameFilters: ["ISO files (*.iso *.gcm)", "All files (*)"]
        onAccepted: isoPathField.text = String(selectedFile).replace("file://", "")
    }

    FolderDialog {
        id: folderFileDialog
        title: "Select comparison folder"
        onAccepted: folderPathField.text = String(selectedFolder).replace("file://", "")
    }
}
