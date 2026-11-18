// Screenshot of the stage to ARG (a path in the VM).
(async () => {
    const {default: Shell} = await import('gi://Shell');
    const {default: Gio} = await import('gi://Gio');
    const shooter = new Shell.Screenshot();
    const file = Gio.File.new_for_path(ARG);
    const stream = file.replace(null, false, Gio.FileCreateFlags.NONE, null);
    await new Promise((resolve, reject) => shooter.screenshot(false, stream, (o, res) => {
        try { o.screenshot_finish(res); resolve(); } catch (e) { reject(e); }
    }));
    stream.close(null);
    return ARG;
})()
