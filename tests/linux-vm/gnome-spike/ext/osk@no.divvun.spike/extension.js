import Gio from 'gi://Gio';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

export default class OskLayoutsExtension extends Extension {
    enable() {
        this._resource = Gio.Resource.load(`${this.path}/osk-layouts.gresource`);
        Gio.resources_register(this._resource);
    }

    disable() {
        Gio.resources_unregister(this._resource);
        this._resource = null;
    }
}
