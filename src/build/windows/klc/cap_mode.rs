pub fn derive_cap_mode() {}

/*
pub(super) fn derive_cap_mode(row: &Row) -> CapMode {
    if !row.caps.is_none() && row.default != row.caps && row.shift != row.caps {
        return CapMode::SGCap; //if nothing seems to take caps lock
    } else if row.caps.is_none() {
        let mut c = 0;
        if row.default != row.shift {
            c += 1;
        }
        if row.alt != row.alt_shift {
            c += 4;
        }
        CapMode::Column(c)
    } else {
        let mut c = 0;
        if row.caps == row.shift {
            c += 1;
        }
        if row.alt_caps == row.alt_shift {
            c += 4;
        }
        CapMode::Column(c)
    }
}
*/
