#!/usr/bin/env python3
# Writes layout/osk/test-se.json in the shape of ldml.linux.touch.gnome.
import json, sys, os
def t(s, label=None, *cands):
    k = {"strings": [s, *cands]}
    if label: k["label"] = label
    return k
def dead(keyval, label):
    return {"keyval": keyval, "label": label}
shift_to = lambda lvl: {"action": "levelSwitch", "iconName": "osk-shift-symbolic", "level": lvl, "width": 1.5}
bksp = {"action": "delete", "iconName": "osk-delete-symbolic", "width": 1.5}
def bottom(to, label):
    return [{"action": "levelSwitch", "label": label, "level": to, "width": 1.5},
            {"action": "emoji", "iconName": "osk-emoji-picker-symbolic"},
            {"strings": [" "], "width": 5},
            {"action": "languageMenu", "iconName": "osk-layout-symbolic"},
            {"action": "hide", "iconName": "osk-hide-symbolic"},
            {"iconName": "osk-enter-symbolic", "keyval": "0xff0d", "width": 1.5}]
default = [
    [t("á"), t("š"), t("e"), t("r"), t("t"), t("ŧ"), t("å"), dead("0xfe51", "´"), dead("0xfe57", "¨"), bksp],
    [t("a"), t("s"), t("d"), t("đ"), t("ö"), t("ä"), t("ŋá", "ŋá3"), t("Ⓧ", "Ⓧ"), dead("0xfe5c", "˛")],
    [shift_to("shift"), t("ž"), t("č"), t("n"), t("ŋ"), dead("0xfe53", "~"), dead("0xfe5a", "ˇ")],
    bottom("opt", "?123"),
]
# "ŋá" here is U+014B U+0061 U+0301: three scalar values.
default[1][6] = {"strings": ["ŋá"], "label": "ŋá3"}
shift = [
    [t("Á"), t("Š"), t("E"), t("R"), t("T"), t("Ŧ"), t("Å"), dead("0xfe50", "`"), dead("0xfe52", "^"), bksp],
    [t("A"), t("S"), t("D"), t("Đ"), t("Ö"), t("Ä"), {"strings": ["\u014aA\u0301"], "label": "ŊÁ3"}, t("Ⓧ"), dead("0xfe5c", "˛")],
    [shift_to("default"), t("Ž"), t("Č"), t("N"), t("Ŋ"), dead("0xfe53", "~"), dead("0xfe5a", "ˇ")],
    bottom("opt", "?123"),
]
opt = [[t(c) for c in "1234567890"] + [bksp],
       [t(c) for c in "@#€%&-+()"],
       [{"action": "levelSwitch", "label": "=/<", "level": "opt+shift", "width": 1.5}] + [t(c) for c in "*\"':;!?"],
       bottom("default", "ABC")]
optshift = [[t(c) for c in "~`|•√π÷×¶∆"] + [bksp],
            [t(c) for c in "£¥$¢^°={}"],
            [{"action": "levelSwitch", "label": "?123", "level": "opt", "width": 1.5}] + [t(c) for c in "\\©®™℅[]"],
            bottom("default", "ABC")]
doc = {"levels": [
    {"level": "default", "mode": "default", "rows": default},
    {"level": "shift", "mode": "latched", "rows": shift},
    {"level": "opt", "mode": "locked", "rows": opt},
    {"level": "opt+shift", "mode": "locked", "rows": optshift}],
    "locale": "se", "name": "Test davvisámegiella"}
out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "osk", "test-se.json")
with open(out, "w", encoding="utf-8") as f:
    json.dump(doc, f, ensure_ascii=False, separators=(",", ":"))
print(out)
