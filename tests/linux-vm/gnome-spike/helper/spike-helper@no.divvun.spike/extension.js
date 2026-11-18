import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

export default class SpikeHelper extends Extension {
    enable() {
        global.context.unsafe_mode = true;
        console.log('spike-helper: unsafe mode on');
    }

    disable() {
        console.log('spike-helper: disabled');
    }
}
