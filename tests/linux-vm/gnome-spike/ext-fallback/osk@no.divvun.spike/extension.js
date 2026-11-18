import Gio from 'gi://Gio';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const PREFIX = 'resource:///org/gnome/shell/osk-layouts/';

export default class OskLayoutsExtension extends Extension {
    enable() {
        const dir = this.dir.get_child('osk-layouts');
        const original = Gio.File.new_for_uri;
        this._original = original;
        Gio.File.new_for_uri = uri => {
            if (uri.startsWith(PREFIX)) {
                const file = dir.get_child(uri.slice(PREFIX.length));
                if (file.query_exists(null))
                    return file;
            }
            return original(uri);
        };
    }

    disable() {
        Gio.File.new_for_uri = this._original;
        this._original = null;
    }
}
