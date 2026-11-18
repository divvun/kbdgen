(() => { const w = global.get_window_actors().find(a => a.meta_window.title === ARG).meta_window; const r = w.get_frame_rect(); return [r.x, r.y, r.width, r.height]; })()
