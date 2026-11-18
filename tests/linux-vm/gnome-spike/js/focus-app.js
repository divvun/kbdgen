// Activates the window whose title is ARG.
(() => {
    const a = global.get_window_actors().find(w => w.meta_window.title === ARG);
    a.meta_window.activate(global.get_current_time());
    return a.meta_window.title;
})()
