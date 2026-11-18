import QtQuick

// Horizontal track with a proportional fill, shared by progress and usage bars.
Rectangle {
    id: root
    property real value: 0 // percent, 0..100
    property real warningThreshold: 75
    property real dangerThreshold: 90
    property color fillColor: value >= dangerThreshold ? theme.danger
                            : (value >= warningThreshold ? theme.warning : theme.accent)
    property color trackColor: theme.surfaceTrack
    property int minFillWidth: 0
    property bool showAtZero: false
    property int animationMs: 180

    implicitHeight: Style.gapSm
    implicitWidth: 0
    radius: height / 2
    color: trackColor

    Rectangle {
        height: parent.height
        radius: parent.radius
        color: root.fillColor
        width: {
            const pct = Math.max(0, Math.min(100, root.value))
            if (pct <= 0 && !root.showAtZero)
                return 0
            return Math.max(root.minFillWidth, root.width * (pct / 100))
        }

        Behavior on width {
            enabled: root.animationMs > 0
            NumberAnimation { duration: root.animationMs }
        }
    }
}
