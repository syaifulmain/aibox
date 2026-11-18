import QtQuick
import QtQuick.Layouts

// Developer overlay (Ctrl+Shift+T): displays the matching `theme.<token>`
// name and hex value for the element under the cursor.
Item {
    id: debugRoot
    property QtObject app: null
    property Item targetRoot: null
    property bool active: false

    anchors.fill: parent
    visible: active
    z: 999

    property real pointerX: 0
    property real pointerY: 0
    property string detectedToken: ""
    property string detectedHex: ""

    function tokenLookup(hexColor) {
        if (!hexColor || hexColor.length === 0)
            return ""
        const target = String(hexColor).toLowerCase()
        const pairs = [
            ["surfaceCanvas", theme.surfaceCanvas],
            ["surfaceCard", theme.surfaceCard],
            ["surfaceSidebar", theme.surfaceSidebar],
            ["surfaceInput", theme.surfaceInput],
            ["surfaceRaised", theme.surfaceRaised],
            ["surfaceSelected", theme.surfaceSelected],
            ["surfaceTrack", theme.surfaceTrack],
            ["surfaceOverlay", theme.surfaceOverlay],
            ["borderSubtle", theme.borderSubtle],
            ["borderStrong", theme.borderStrong],
            ["borderFocus", theme.borderFocus],
            ["textPrimary", theme.textPrimary],
            ["textSecondary", theme.textSecondary],
            ["textMuted", theme.textMuted],
            ["textDisabled", theme.textDisabled],
            ["textOnAccent", theme.textOnAccent],
            ["accent", theme.accent],
            ["accentText", theme.accentText],
            ["accentSurface", theme.accentSurface],
            ["accentBorder", theme.accentBorder],
            ["danger", theme.danger],
            ["dangerText", theme.dangerText],
            ["dangerSurface", theme.dangerSurface],
            ["dangerBorder", theme.dangerBorder],
            ["success", theme.success],
            ["successText", theme.successText],
            ["successSurface", theme.successSurface],
            ["successBorder", theme.successBorder],
            ["warning", theme.warning],
            ["warningText", theme.warningText],
            ["warningSurface", theme.warningSurface],
            ["warningBorder", theme.warningBorder],
            ["info", theme.info],
            ["infoText", theme.infoText],
            ["infoSurface", theme.infoSurface],
            ["infoBorder", theme.infoBorder],
            ["chart1", theme.chart1],
            ["chart2", theme.chart2],
            ["chart3", theme.chart3],
            ["chart4", theme.chart4]
        ]
        const matches = []
        for (let i = 0; i < pairs.length; i++) {
            if (String(pairs[i][1]).toLowerCase() === target)
                matches.push("theme." + pairs[i][0])
        }
        return matches.join(" / ")
    }

    function inspectAt(px, py) {
        pointerX = px
        pointerY = py
        if (!targetRoot)
            return

        let item = targetRoot
        let localX = px
        let localY = py
        let deepest = item

        for (let depth = 0; depth < 24; depth++) {
            const child = item.childAt(localX, localY)
            if (!child || child === debugRoot || !child.visible)
                break
            const mapped = item.mapToItem(child, localX, localY)
            deepest = child
            item = child
            localX = mapped.x
            localY = mapped.y
        }

        let hex = ""
        if (deepest && deepest.color !== undefined && !Qt.colorEqual(deepest.color, "transparent")) {
            hex = String(deepest.color)
        }
        if (hex.length === 0 && deepest && deepest.parent && deepest.parent.color !== undefined
                && !Qt.colorEqual(deepest.parent.color, "transparent")) {
            hex = String(deepest.parent.color)
        }
        detectedHex = hex
        detectedToken = tokenLookup(hex)
    }

    HoverHandler {
        id: hoverHandler
        onPointChanged: debugRoot.inspectAt(point.position.x, point.position.y)
    }

    Rectangle {
        id: badge
        visible: debugRoot.detectedHex.length > 0
        x: Math.min(Math.max(Style.space8, debugRoot.pointerX + Style.space14), debugRoot.width - width - Style.space8)
        y: Math.min(Math.max(Style.space8, debugRoot.pointerY + Style.space16), debugRoot.height - height - Style.space8)
        width: badgeCol.implicitWidth + Style.space16
        height: badgeCol.implicitHeight + Style.space10
        radius: Math.min(Style.radius5, theme.radius)
        color: theme.surfaceOverlay
        border.width: Style.hairline
        border.color: theme.borderFocus

        ColumnLayout {
            id: badgeCol
            anchors.centerIn: parent
            spacing: Style.space2

            Text {
                text: debugRoot.detectedToken.length > 0
                      ? debugRoot.detectedToken
                      : qsTr("Custom / Unmapped")
                color: theme.accentText
                font.family: Style.fontMono
                font.pixelSize: Style.font11
                font.weight: Font.Medium
            }

            Text {
                text: debugRoot.detectedHex + "  (" + theme.mode + (theme.highContrast ? ", HC" : "") + ")"
                color: theme.textSecondary
                font.family: Style.fontMono
                font.pixelSize: Style.font10
            }
        }
    }
}
