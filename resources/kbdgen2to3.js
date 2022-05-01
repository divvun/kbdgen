// To run, use `deno run --allow-read resources/kbdgen2to3.js <path/to/layout.yaml>` 
// https://deno.land to get deno.

import yaml from 'https://cdn.skypack.dev/yaml'

const [filename] = Deno.args

const text = await Deno.readTextFile(filename)
const { displayNames, modes, deadKeys, longpress, transforms, strings, decimal, space, targets } = yaml.parse(text)

const newDoc = {}

newDoc.displayNames = displayNames
newDoc.decimal = decimal

if (modes.mac != null) {
    newDoc.macOS = {
        config: {},
        primary: {
            layers: {},
        },
        deadKeys: {},
        space: {}
    }

    newDoc.macOS.primary.layers = fixLayer(modes.mac)
    newDoc.macOS.deadKeys = Object.entries(fixLayer(deadKeys.mac)).reduce((acc, [k, v]) => {
        acc[k] = `["${v.join("\", \"")}"]`
        return acc
    }, {})
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
    newDoc.windows.deadKeys = Object.entries(fixLayer(deadKeys.win)).reduce((acc, [k, v]) => {
        acc[k] = `["${v.join("\", \"")}"]`
        return acc
    }, {})
    newDoc.windows.space = space.win ? fixLayer(space.win) : undefined
    newDoc.windows.config = (targets ?? {}).win
}

newDoc.transforms = transforms
newDoc.longpress = longpress

console.log(yaml.stringify(newDoc).replace(/('\[|\]')/g, (match) => {
    if (match.startsWith("'")) {
        return "["
    } else {
        return "]"
    }
}))

function fixLayer(layer) {
    if (layer["caps+alt"]) {
        layer["alt+caps"] = layer["caps+alt"]
        delete layer["caps+alt"]
    }
    return layer
}
