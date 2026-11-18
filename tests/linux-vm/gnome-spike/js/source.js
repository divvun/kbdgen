// Activates the input source ARG = "type:id" as the user would.
(async () => {
    const {getInputSourceManager} = await import('resource:///org/gnome/shell/ui/status/keyboard.js');
    const ism = getInputSourceManager();
    const [type, id] = ARG.split(':');
    const s = Object.values(ism.inputSources).find(x => x.type === type && x.id === id);
    if (!s) return `no source ${ARG}`;
    s.activate(true);
    return `activated ${s.type}:${s.id} xkbId=${s.xkbId}`;
})()
