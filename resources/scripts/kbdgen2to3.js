// To run, use `deno run --allow-read resources/scripts/kbdgen2to3.js <path/to/layout.yaml>` 
// https://deno.land to get deno.

import yaml from 'https://cdn.skypack.dev/yaml'

const [filename] = Deno.args

const text = await Deno.readTextFile(filename)
const { displayNames, modes, deadKeys, longpress, transforms, strings, decimal, space, targets } = yaml.parse(text)

const newDoc = {}

newDoc.displayNames = displayNames
newDoc.decimal = decimal

if (modes.android != null) {
    newDoc.android = {
        config: {},
        primary: {
            layers: {},
        },
    }

    newDoc.android.primary.layers = modes.android
    if (targets.android != null) {
        if (targets.android.spellerPackageKey != null) {
            newDoc.android.config.spellerPackageKey = targets.android.spellerPackageKey
        }
        if (targets.android.spellerPath != null) {
            newDoc.android.config.spellerPath = targets.android.spellerPath
        }
        if (targets.android.styles != null) {
            const { shift, backspace } = targets.android.styles.phone.actions
            // TODO: handle tablet
    
            const defaultLayer = newDoc.android.primary.layers.default.split("\n").map(x => x.trim().split(" "))
            const shiftLayer = newDoc.android.primary.layers.shift.split("\n").map(x => x.trim().split(" "))
    
            const shiftKeyIndex = shift[0] - 1
            const backspaceKeyIndex = backspace[0] - 1
    
            defaultLayer[shiftKeyIndex].unshift("\\s{shift}")
            defaultLayer[backspaceKeyIndex].push("\\s{backspace}")
    
            shiftLayer[shiftKeyIndex].unshift("\\s{shift}")
            shiftLayer[backspaceKeyIndex].push("\\s{backspace}")
    
            newDoc.android.primary.layers.default = defaultLayer.map(x => x.join(" ")).join("\n")
            newDoc.android.primary.layers.shift = shiftLayer.map(x => x.join(" ")).join("\n")
        } else {
            const defaultLayer = newDoc.android.primary.layers.default.split("\n").map(x => x.trim().split(" "))
            const shiftLayer = newDoc.android.primary.layers.shift.split("\n").map(x => x.trim().split(" "))
    
            defaultLayer[defaultLayer.length - 2].unshift("\\s{shift}")
            defaultLayer[defaultLayer.length - 2].push("\\s{backspace}")
    
            shiftLayer[shiftLayer.length - 2].unshift("\\s{shift}")
            shiftLayer[shiftLayer.length - 2].push("\\s{backspace}")
    
            newDoc.android.primary.layers.default = defaultLayer.map(x => x.join(" ")).join("\n")
            newDoc.android.primary.layers.shift = shiftLayer.map(x => x.join(" ")).join("\n")
        }
    } else {
        const defaultLayer = newDoc.android.primary.layers.default.split("\n").map(x => x.trim().split(" "))
        const shiftLayer = newDoc.android.primary.layers.shift.split("\n").map(x => x.trim().split(" "))

        defaultLayer[defaultLayer.length - 2].unshift("\\s{shift}")
        defaultLayer[defaultLayer.length - 2].push("\\s{backspace}")

        shiftLayer[shiftLayer.length - 2].unshift("\\s{shift}")
        shiftLayer[shiftLayer.length - 2].push("\\s{backspace}")

        newDoc.android.primary.layers.default = defaultLayer.map(x => x.join(" ")).join("\n")
        newDoc.android.primary.layers.shift = shiftLayer.map(x => x.join(" ")).join("\n")
    }

    if (deadKeys != null && deadKeys.android != null) {
        newDoc.android.deadKeys = Object.entries(fixLayer(deadKeys.android)).reduce((acc, [k, v]) => {
            acc[k] = `["${v.join("\", \"")}"]`
            return acc
        }, {})
    }
}

if (modes.ios != null) {
    newDoc.iOS = {
        config: {},
        primary: {
            layers: {},
        },
        "iPad-9in": {
            layers: {},
        },
        "iPad-12in": {
            layers: {},
        },
    }

    const defaultLayer = modes.ios.default.split("\n").map(x => x.trim().split(/\s+/).join(" ")).join("\n")
    const shiftLayer = modes.ios.shift.split("\n").map(x => x.trim().split(/\s+/).join(" ")).join("\n")
    const symbols1Layer = modes.ios["symbols-1"].split("\n").map(x => x.trim().split(/\s+/).join(" ")).join("\n")
    const symbols2Layer  = modes.ios["symbols-2"].split("\n").map(x => x.trim().split(/\s+/).join(" ")).join("\n")

    newDoc.iOS.primary.layers.default = defaultLayer
    newDoc.iOS.primary.layers.shift = shiftLayer
    newDoc.iOS.primary.layers["symbols-1"] = symbols1Layer
    newDoc.iOS.primary.layers["symbols-2"] = symbols2Layer

    if (deadKeys != null && deadKeys.ios != null) {
        newDoc.iOS.deadKeys = Object.entries(fixLayer(deadKeys.ios)).reduce((acc, [k, v]) => {
            acc[k] = `["${v.join("\", \"")}"]`
            return acc
        }, {})
    }

    if (targets.ios != null && targets.ios.spellerPackageKey != null) {
        newDoc.iOS.config.spellerPackageKey = targets.ios.spellerPackageKey
    }

    if (targets.ios != null && targets.ios.spellerPath != null) {
        newDoc.iOS.config.spellerPath = targets.ios.spellerPath
    }


    if (modes["ipad-9in"] != null) {
        newDoc.iOS["iPad-9in"].layers = modes["ipad-9in"]
    }

    if (modes["ipad-12in"] != null) {
        newDoc.iOS["iPad-12in"].layers = modes["ipad-12in"]
    }
}

if (modes.mac != null) {
    newDoc.macOS = {
        config: {},
        primary: {
            layers: {},
        },
        space: {}
    }

    newDoc.macOS.primary.layers = fixLayer(modes.mac)
    if (deadKeys != null && deadKeys.mac != null) {
        newDoc.macOS.deadKeys = Object.entries(fixLayer(deadKeys.mac)).reduce((acc, [k, v]) => {
            acc[k] = `["${v.join("\", \"")}"]`
            return acc
        }, {})
    }
    newDoc.macOS.space = space.mac ? fixLayer(space.mac) : undefined
    newDoc.macOS.config = (targets ?? {}).mac
}

if (modes.win != null) {
    newDoc.windows = {
        config: {},
        primary: {
            layers: {},
        },
        deadKeys: {},
        space: {}
    }

    newDoc.windows.primary.layers = fixLayer(modes.win)
    if (deadKeys != null && deadKeys.win != null) {
        newDoc.windows.deadKeys = Object.entries(fixLayer(deadKeys.win)).reduce((acc, [k, v]) => {
            acc[k] = `["${v.join("\", \"")}"]`
            return acc
        }, {})
    }
    newDoc.windows.space = space.win ? fixLayer(space.win) : undefined
    newDoc.windows.config = (targets ?? {}).win
}

if (modes.chrome != null) {
    newDoc.chromeOS = {
        config: targets.chrome,
        primary: {
            layers: modes.chrome
        },
    }

    if (deadKeys != null && deadKeys.chrome != null) {
        newDoc.chromeOS.deadKeys = deadKeys.chrome
    }
}

newDoc.transforms = transforms
newDoc.longpress = longpress

if (strings != null) {
    newDoc.keyNames = strings
}

const output = yaml.stringify(newDoc, null, {
    version: "1.1",
    lineWidth: 10000000,
    singleQuote: true
}).replace(/('\[(.*?)\]')/g, (match) => {
    if (match === "'[\"\"]'") {
        return "[]"
    }
    const x = match.substring(3, match.length - 3).split("\", \"")
    return `['${x.join("', '")}']`
}).replace(/: >\n((?:\s+.+\n\n)*)/mg, (match) => {
    return match.replaceAll("\n\n", "\n").replace(": >", ": |")
}).replace(/[\u{00a0}\u{00ad}]/ug, (match) => {
    return `\\u00${match.codePointAt(0).toString(16).toUpperCase()}`
}).replace(/\s*\w+: \{\}\n/mg, "\n")

console.log(output)

function fixLayer(layer) {
    if (layer["caps+alt"]) {
        layer["alt+caps"] = layer["caps+alt"]
        delete layer["caps+alt"]
    }
    return layer
}
