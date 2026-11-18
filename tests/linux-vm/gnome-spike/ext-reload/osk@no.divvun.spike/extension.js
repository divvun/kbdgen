import Gio from 'gi://Gio';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import {getInputSourceManager} from 'resource:///org/gnome/shell/ui/status/keyboard.js';

// The on-screen keyboard reads its layout only when the input source, the
// source list or the field's purpose changes, so both reload the sources.
export default class OskLayoutsExtension extends Extension {
    enable() {
        this._resource = Gio.Resource.load(`${this.path}/osk-layouts.gresource`);
        Gio.resources_register(this._resource);
        getInputSourceManager().reload();
    }

    disable() {
        Gio.resources_unregister(this._resource);
        this._resource = null;
        getInputSourceManager().reload();
    }
}
