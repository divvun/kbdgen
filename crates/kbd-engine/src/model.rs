//! The loaded model and the engine's entry point, `Model::key`.

use alloc::string::String;
use alloc::vec::Vec;

use kbd_model::{
    DecodeError, EmojiKey, ExtraModifierKey, Invariant, Key as ModelKey, KeyIndex, Keyboard,
    NfdCheck, Normalization, Text, TextElem, TransformList,
};

use crate::api::{
    Action, BackspacePolicy, Context, Error, Gesture, Key, KeyEvent, ModifierState, Options, State,
    trailing_markers,
};
use crate::modifiers::{B00_SCAN_CODE, apply_extra_bindings, is_shortcut, select_layer};
use crate::normalize::{is_starter, nfd_chars, nfd_elems, to_output_form};
use crate::transforms::{GroupInfo, group_infos, run_groups};

/// The NFD check that validation uses: ICU4X's with the `normalization`
/// feature, none without it, so that a keyboard with normalization
/// `Enabled` fails validation with `NfdUnchecked`.
#[cfg(feature = "normalization")]
const NFD_CHECK: Option<&dyn NfdCheck> = Some(&crate::normalize::IcuNfd);

#[cfg(not(feature = "normalization"))]
const NFD_CHECK: Option<&dyn NfdCheck> = None;

/// Maps the validation failure of an `Enabled` keyboard without an NFD
/// check to the refusal of `ldml.crate.engine`.
fn invariant_error(error: kbd_model::InvariantError) -> Error {
    if error.invariant == Invariant::NfdUnchecked {
        Error::NormalizationUnsupported
    } else {
        Error::Invalid(error)
    }
}

// [spec:kbdgen:def:ldml.engine.api+1]
// [spec:kbdgen:def:ldml.scope.v1]
/// A validated keyboard ready to process events, with the options that the
/// platform chooses. A `Model` is immutable: every event is a pure function
/// of the model, a host-owned [`State`], a [`Context`] and a [`KeyEvent`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    keyboard: Keyboard,
    /// `keyboard.context_len`, at most 64.
    context_len: usize,
    options: Options,
    simple: Vec<GroupInfo>,
    backspace: Vec<GroupInfo>,
}

/// The working context of one event (`ldml.engine.context`).
struct Working {
    /// T: the scalar values of `Context.text` the engine uses.
    t: Vec<char>,
    /// C: the context with the state's markers, NFD when normalization is
    /// `Enabled`.
    c: Vec<TextElem>,
    at_start: bool,
}

/// What an event does before the edit is computed.
enum Outcome {
    Pass,
    /// (0, "", current preedit) with the state unchanged.
    Consume,
    /// The processed context C′, and the touch layer to show next.
    Changed(Vec<TextElem>, Option<String>),
}

impl Model {
    // [spec:kbdgen:req:tsf.data.version]
    /// Decodes a `DVKB` model (`ldml.model.encoding`) with default
    /// [`Options`]; [`Model::with_options`] chooses others. An unknown major
    /// version is refused and a higher minor version accepted.
    pub fn from_bytes(bytes: &[u8]) -> Result<Model, Error> {
        let keyboard = Keyboard::from_bytes(bytes, NFD_CHECK).map_err(|e| match e {
            DecodeError::Invariant(error) => invariant_error(error),
            other => Error::Decode(other),
        })?;
        Model::build(keyboard, Options::default())
    }

    /// The same model with `options`.
    pub fn with_options(self, options: Options) -> Model {
        Model { options, ..self }
    }

    // [spec:kbdgen:req:ldml.crate.engine+1]
    /// Validates `keyboard` and prepares it. Without the `normalization`
    /// feature, a keyboard with normalization `Enabled` is refused with
    /// [`Error::NormalizationUnsupported`].
    pub fn from_keyboard(keyboard: Keyboard, options: Options) -> Result<Model, Error> {
        keyboard.validate(NFD_CHECK).map_err(invariant_error)?;
        Model::build(keyboard, options)
    }

    fn build(keyboard: Keyboard, options: Options) -> Result<Model, Error> {
        if cfg!(not(feature = "normalization")) && keyboard.normalization == Normalization::Enabled
        {
            return Err(Error::NormalizationUnsupported);
        }
        let simple = group_infos(&keyboard, TransformList::Simple).map_err(Error::Invalid)?;
        let backspace = group_infos(&keyboard, TransformList::Backspace).map_err(Error::Invalid)?;
        Ok(Model {
            context_len: usize::from(keyboard.context_len),
            keyboard,
            options,
            simple,
            backspace,
        })
    }

    /// The keyboard, for hosts that draw keys, labels and touch layers.
    pub fn keyboard(&self) -> &Keyboard {
        &self.keyboard
    }

    pub fn options(&self) -> Options {
        self.options
    }

    /// How many scalar values before the caret the engine reads; at most
    /// 64 (`ldml.model.context-len`).
    pub fn context_len(&self) -> usize {
        self.context_len
    }

    /// The preserved key of `ldml.model.emoji`, which opens the emoji
    /// picker.
    pub fn preserved_keys(&self) -> Option<EmojiKey> {
        self.keyboard.emoji.key
    }

    // [spec:kbdgen:sem:ldml.engine.touch]
    /// The touch set with the greatest `min_device_width` ≤ `width`, a set
    /// without one counting as 0. With no touch sets, the hardware set
    /// presented as touch, as set 0.
    pub fn touch_set_for_width(&self, width: u16) -> Option<usize> {
        if self.keyboard.touch.is_empty() {
            return self.keyboard.hardware.as_ref().map(|_| 0);
        }
        self.keyboard
            .touch
            .iter()
            .enumerate()
            .filter(|(_, set)| set.min_device_width.unwrap_or(0) <= width)
            .max_by_key(|(_, set)| set.min_device_width.unwrap_or(0))
            .map(|(i, _)| i)
    }

    /// The touch set named `name` (`phone`, `tablet`, …).
    pub fn touch_set_by_name(&self, name: &str) -> Option<usize> {
        self.keyboard
            .touch
            .iter()
            .position(|set| set.name.as_deref() == Some(name))
    }

    /// The names of the markers in the trailing marker run of the state, in
    /// order: the pending dead keys.
    pub fn pending_markers(&self, state: &State) -> Vec<&str> {
        trailing_markers(state.tail.elements())
            .filter_map(|m| self.keyboard.marker_name(m))
            .collect()
    }

    fn enabled(&self) -> bool {
        self.keyboard.normalization == Normalization::Enabled
    }

    // [spec:kbdgen:req:ldml.engine.contract+1]
    // [spec:kbdgen:req:ldml.engine.tsf+1]
    /// What `event` does in `context` from `state`: the action and the next
    /// state. On `Pass` the state is returned unchanged.
    ///
    /// This is a pure function: deterministic, with no I/O, clock or
    /// global state, so a host may ask what a key would do without
    /// committing to it. It never panics, and its work is polynomial in the
    /// model size and `context_len`. `Context.authoritative` does not
    /// affect the result.
    pub fn key(&self, state: &State, context: &Context, event: &KeyEvent) -> (Action, State) {
        let working = self.working(state, context);
        let outcome = match &event.key {
            Key::Scan(code) => self.scan(&working, *code, event.modifiers),
            Key::Decimal => self.decimal(&working, event.modifiers),
            Key::Backspace => self.backspace(&working, event.modifiers),
            Key::Touch {
                set,
                layer,
                row,
                col,
                gesture,
            } => match self.touch_key(*set, *layer, *row, *col) {
                Some(key) => self.gesture(&working, key, gesture),
                None => Outcome::Pass,
            },
            Key::Id { id, gesture } => match self.keyboard.key_index(id) {
                Some(key) => self.gesture(&working, key, gesture),
                None => Outcome::Consume,
            },
            Key::Emit(text) => self.insert(&working, &Text::from(text.as_str()), None),
            Key::Commit => self.commit(&working),
        };
        match outcome {
            Outcome::Pass => (Action::Pass, state.clone()),
            Outcome::Consume => (
                Action::Edit {
                    delete: 0,
                    insert: String::new(),
                    preedit: self.preedit(&working.c),
                    layer: None,
                },
                state.clone(),
            ),
            Outcome::Changed(c, layer) => self.finish(&working, c, layer),
        }
    }

    // [spec:kbdgen:sem:ldml.engine.context+1]
    /// The working context C. X is the last `context_len` scalar values of
    /// `Context.text`, in NFD when normalization is `Enabled`. If X ends
    /// with the plain text of the state's `tail`, C is X with that suffix
    /// replaced by `tail`, keeping its markers; otherwise C is X without
    /// markers, because the context changed.
    fn working(&self, state: &State, context: &Context) -> Working {
        let all: Vec<char> = context.text.chars().collect();
        let skip = all.len().saturating_sub(self.context_len);
        let t: Vec<char> = all.into_iter().skip(skip).collect();
        let x: Vec<char> = if self.enabled() {
            nfd_chars(&t)
        } else {
            t.clone()
        };
        let tail = state.tail.elements();
        let tail_plain: Vec<char> = state.tail.chars().collect();
        let mut c: Vec<TextElem>;
        if x.ends_with(&tail_plain) {
            let keep = x.len().saturating_sub(tail_plain.len());
            c = x.iter().take(keep).copied().map(TextElem::Char).collect();
            c.extend_from_slice(tail);
        } else {
            c = x.into_iter().map(TextElem::Char).collect();
        }
        Working {
            t,
            c,
            at_start: context.at_start && skip == 0,
        }
    }

    // [spec:kbdgen:sem:ldml.engine.hardware]
    /// A hardware key: the `B00` extra modifier is consumed; a key outside
    /// the form passes; a shortcut passes; the selected layer's key at the
    /// scan code's position is inserted, or the event is consumed when that
    /// position has no key, a gap or an empty output. Hardware keys ignore
    /// `layer_id` and gestures.
    fn scan(&self, w: &Working, code: u8, modifiers: ModifierState) -> Outcome {
        let windows = &self.keyboard.windows;
        if code == B00_SCAN_CODE && windows.extra_modifiers.contains(&ExtraModifierKey::B00) {
            return Outcome::Consume;
        }
        let Some(hardware) = &self.keyboard.hardware else {
            return Outcome::Pass;
        };
        let Some((row, col)) = hardware.form.position_of(code) else {
            return Outcome::Pass;
        };
        let m = apply_extra_bindings(windows, modifiers);
        if is_shortcut(m, self.keyboard.host.or(self.options.host)) {
            return Outcome::Pass;
        }
        let Some(layer) = select_layer(hardware, m) else {
            return Outcome::Pass;
        };
        let key = hardware
            .layers
            .get(layer)
            .and_then(|l| l.key_at(row, col))
            .and_then(|k| self.keyboard.key(k));
        match key {
            Some(key) if !key.gap && !key.output.is_empty() => self.insert(w, &key.output, None),
            _ => Outcome::Consume,
        }
    }

    // [spec:kbdgen:sem:ldml.engine.decimal]
    /// The numpad decimal key inserts `decimal` whatever the other
    /// modifiers are; it passes when there is none or for a shortcut.
    fn decimal(&self, w: &Working, modifiers: ModifierState) -> Outcome {
        let m = apply_extra_bindings(&self.keyboard.windows, modifiers);
        match &self.keyboard.decimal {
            Some(decimal) if !is_shortcut(m, self.keyboard.host.or(self.options.host)) => {
                self.insert(w, decimal, None)
            }
            _ => Outcome::Pass,
        }
    }

    /// The key at a touch position: in touch set `set`, or, for a keyboard
    /// without touch sets, in its hardware set presented as touch.
    fn touch_key(&self, set: usize, layer: usize, row: usize, col: usize) -> Option<KeyIndex> {
        if self.keyboard.touch.is_empty() {
            if set != 0 {
                return None;
            }
            return self
                .keyboard
                .hardware
                .as_ref()?
                .layers
                .get(layer)?
                .key_at(row, col);
        }
        self.keyboard
            .touch
            .get(set)?
            .layers
            .get(layer)?
            .rows
            .get(row)?
            .get(col)
            .copied()
    }

    // [spec:kbdgen:sem:ldml.engine.touch.gestures+1]
    // [spec:kbdgen:sem:ldml.engine.test-keys+1]
    /// Resolves `gesture` on `key` and inserts the resolved key's output,
    /// returning its `layer_id` after output processing. A gap or a role
    /// key passes, since role keys are the host's own; a gesture with no
    /// target consumes the event. Gestures of the target key are ignored.
    fn gesture(&self, w: &Working, key: KeyIndex, gesture: &Gesture) -> Outcome {
        let Some(pressed) = self.keyboard.key(key) else {
            return Outcome::Consume;
        };
        if pressed.gap || pressed.role.is_some() {
            return Outcome::Pass;
        }
        let target = match gesture {
            Gesture::Tap => Some(key),
            Gesture::LongPress(0) => pressed.long_press_default,
            Gesture::LongPress(n) => pressed.long_press.get(n.saturating_sub(1)).copied(),
            Gesture::MultiTap(0) => None,
            Gesture::MultiTap(n) => {
                let cycle = pressed.multi_tap.len().saturating_add(1);
                match n.saturating_sub(1).checked_rem(cycle) {
                    Some(0) => Some(key),
                    Some(i) => pressed.multi_tap.get(i.saturating_sub(1)).copied(),
                    None => None,
                }
            }
            Gesture::Flick(directions) => pressed
                .flick
                .and_then(|f| self.keyboard.flicks.get(usize::from(f)))
                .and_then(|f| f.segments.iter().find(|s| &s.directions == directions))
                .map(|s| s.key),
        };
        match target.and_then(|k| self.keyboard.key(k)) {
            Some(ModelKey {
                output, layer_id, ..
            }) => self.insert(w, output, layer_id.clone()),
            None => Outcome::Consume,
        }
    }

    // [spec:kbdgen:sem:ldml.engine.insert]
    /// Appends `output` to C, normalizes C when normalization is `Enabled`,
    /// and runs the `simple` groups unless `output` is empty, so keys
    /// without output, such as layer switches, never rewrite text.
    fn insert(&self, w: &Working, output: &Text, layer: Option<String>) -> Outcome {
        let mut c = w.c.clone();
        c.extend_from_slice(output.elements());
        if self.enabled() {
            c = nfd_elems(&c);
        }
        if !output.is_empty() {
            run_groups(
                &self.keyboard,
                &self.keyboard.simple,
                &self.simple,
                &mut c,
                w.at_start,
            );
        }
        Outcome::Changed(c, layer)
    }

    // [spec:kbdgen:sem:ldml.engine.backspace+1]
    /// Backspace: Left or Right Shift insert LRM or RLM when
    /// `windows.lrm_rlm` is set; a shortcut passes; otherwise the
    /// `backspace` groups run over C with nothing appended, and if no rule
    /// matched the default applies. The `simple` groups are not run.
    fn backspace(&self, w: &Working, modifiers: ModifierState) -> Outcome {
        if self.keyboard.windows.lrm_rlm {
            if modifiers.shift_l {
                return self.insert(w, &Text::from("\u{200E}"), None);
            }
            if modifiers.shift_r {
                return self.insert(w, &Text::from("\u{200F}"), None);
            }
        }
        let m = apply_extra_bindings(&self.keyboard.windows, modifiers);
        if is_shortcut(m, self.keyboard.host.or(self.options.host)) {
            return Outcome::Pass;
        }
        let mut c = w.c.clone();
        if run_groups(
            &self.keyboard,
            &self.keyboard.backspace,
            &self.backspace,
            &mut c,
            w.at_start,
        ) {
            return Outcome::Changed(c, None);
        }
        self.default_backspace(&w.c)
    }

    // [spec:kbdgen:sem:ldml.engine.backspace.default]
    /// The default Backspace. `CancelOrPass` removes a trailing marker run
    /// and otherwise passes, so the application deletes as it normally
    /// does. `CodePoint` deletes the last scalar value with the markers
    /// adjoining it on either side; with no scalar value it removes the
    /// markers, or passes when there are none.
    fn default_backspace(&self, c: &[TextElem]) -> Outcome {
        let last_char = c.iter().rposition(|e| matches!(e, TextElem::Char(_)));
        let trailing_start = last_char.map_or(0, |i| i.saturating_add(1));
        match self.options.backspace {
            BackspacePolicy::CancelOrPass => {
                if trailing_start < c.len() {
                    Outcome::Changed(c.iter().take(trailing_start).copied().collect(), None)
                } else {
                    Outcome::Pass
                }
            }
            BackspacePolicy::CodePoint => match last_char {
                Some(i) => {
                    let before = c.get(..i).unwrap_or(&[]);
                    let markers_before = before
                        .iter()
                        .rev()
                        .take_while(|e| matches!(e, TextElem::Marker(_)))
                        .count();
                    let keep = i.saturating_sub(markers_before);
                    Outcome::Changed(c.iter().take(keep).copied().collect(), None)
                }
                None if c.is_empty() => Outcome::Pass,
                None => Outcome::Changed(Vec::new(), None),
            },
        }
    }

    // [spec:kbdgen:sem:ldml.engine.commit+1]
    /// `Commit` replaces the trailing marker run of C with the flush
    /// outputs of its markers and removes every other marker, running no
    /// transforms.
    fn commit(&self, w: &Working) -> Outcome {
        let flush = self.flush_text(&w.c);
        let trailing = trailing_markers(&w.c).count();
        let body = w.c.len().saturating_sub(trailing);
        let mut c: Vec<TextElem> =
            w.c.iter()
                .take(body)
                .filter(|e| matches!(e, TextElem::Char(_)))
                .copied()
                .collect();
        c.extend(flush.chars().map(TextElem::Char));
        if self.enabled() {
            c = nfd_elems(&c);
        }
        Outcome::Changed(c, None)
    }

    /// The concatenated flush outputs of the trailing marker run of `c`.
    /// Markers without a flush output contribute nothing.
    fn flush_text(&self, c: &[TextElem]) -> String {
        trailing_markers(c)
            .filter_map(|m| self.keyboard.flush.get(&m))
            .map(String::as_str)
            .collect()
    }

    // [spec:kbdgen:sem:ldml.engine.preedit+1]
    /// The preedit for context `c`: the flush outputs of its trailing
    /// marker run, which is exactly what `Commit` would insert, in the
    /// output form when normalization is `Enabled`.
    fn preedit(&self, c: &[TextElem]) -> String {
        self.output_form(self.flush_text(c))
    }

    /// Inserted text in the output form; unchanged for `Disabled`
    /// keyboards.
    fn output_form(&self, text: String) -> String {
        if self.enabled() {
            to_output_form(text, self.options.output_form)
        } else {
            text
        }
    }

    // [spec:kbdgen:sem:ldml.engine.output.segment+1]
    // [spec:kbdgen:thm:ldml.engine.no-markers-out]
    /// The edit from T to the processed C′, the preedit, and the new state.
    ///
    /// With `Disabled`, the edit keeps the longest common prefix of T and
    /// plain(C′). With `Enabled`, it keeps the longest prefix T[..i] that
    /// ends at a segment boundary of T whose NFD is a prefix of plain(C′)
    /// ending at a boundary of C′, so normalization touches only text from
    /// the caret's segment onwards; i = 0 is the fallback. `insert` is
    /// built from plain(C′) and `preedit` from flush texts, so neither
    /// holds a marker, and `delete` ≤ |T|. The new `tail` is the end of C′
    /// covering `context_len` scalar values plus the markers among or
    /// after them; when C′ has no more scalar values than that, all of C′.
    fn finish(&self, w: &Working, c: Vec<TextElem>, layer: Option<String>) -> (Action, State) {
        let plain: Vec<char> = c
            .iter()
            .filter_map(|e| match e {
                TextElem::Char(ch) => Some(*ch),
                TextElem::Marker(_) => None,
            })
            .collect();
        let (keep, rest_start) = if self.enabled() {
            self.normalized_prefix(&w.t, &plain)
        } else {
            let common = w.t.iter().zip(&plain).take_while(|(a, b)| a == b).count();
            (common, common)
        };
        let insert: String = plain.iter().skip(rest_start).collect();
        let action = Action::Edit {
            delete: w.t.len().saturating_sub(keep),
            insert: self.output_form(insert),
            preedit: self.preedit(&c),
            layer,
        };
        let state = State {
            tail: Text(self.cut_tail(c)),
        };
        (action, state)
    }

    /// For `Enabled` keyboards, (i, |NFD(T[..i])|) for the largest i of
    /// `ldml.engine.output.segment`.
    fn normalized_prefix(&self, t: &[char], plain: &[char]) -> (usize, usize) {
        for i in (1..=t.len()).rev() {
            let boundary = t.get(i).is_none_or(|c| {
                nfd_chars(core::slice::from_ref(c))
                    .first()
                    .is_none_or(|first| is_starter(*first))
            });
            if !boundary {
                continue;
            }
            let prefix = nfd_chars(t.get(..i).unwrap_or(&[]));
            if !plain.starts_with(&prefix) {
                continue;
            }
            if plain
                .get(prefix.len())
                .is_none_or(|first| is_starter(*first))
            {
                return (i, prefix.len());
            }
        }
        (0, 0)
    }

    /// The new `tail`: the end of `c` covering `context_len` scalar values,
    /// plus any markers among or after them.
    fn cut_tail(&self, c: Vec<TextElem>) -> Vec<TextElem> {
        let limit = self.context_len;
        let mut seen = 0usize;
        let mut start = None;
        for (i, e) in c.iter().enumerate().rev() {
            if let TextElem::Char(_) = e {
                seen = seen.saturating_add(1);
                if seen == limit {
                    start = Some(i);
                    break;
                }
            }
        }
        match start {
            Some(i) if c.iter().take(i).any(|e| matches!(e, TextElem::Char(_))) => {
                c.into_iter().skip(i).collect()
            }
            _ => c,
        }
    }
}
