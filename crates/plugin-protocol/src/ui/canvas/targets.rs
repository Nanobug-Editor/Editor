//! Stable target identities separate editable/input objects from the scene's drawing IDs.
use super::*;

/// One bounded interactive object. Reuse its ID only with a fresh epoch after replacement.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HitTarget {
    /// Optional group-local coordinate space; ancestors' transforms, clips and visibility apply.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space: Option<String>,
    /// Optional native IME/draft session. This cannot reference or copy an editor DocumentSession.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_input: Option<TextInputSession>,
    /// Emit native pointer/wheel events for this hit shape; false makes it keyboard-only.
    #[serde(default = "enabled")]
    pub pointer_events: bool,
    /// Begin native capture synchronously on press, retaining this original target outside its bounds.
    #[serde(default)]
    pub capture_on_down: bool,
    /// Focus a focusable target synchronously on press; true by default.
    #[serde(default = "enabled")]
    pub focus_on_down: bool,
    /// System cursor over the actual hit shape or during capture.
    #[serde(default)]
    pub cursor: CanvasCursor,
    /// Scene-local target ID, independent of drawing and semantic IDs.
    pub id: String,
    /// Nonzero semantic generation, retained across drawing-only updates.
    pub epoch: u64,
    /// Hit shape in Canvas content coordinates, projected exactly like scene geometry.
    pub geometry: SceneGeometry,
    /// May receive native keyboard focus independently of the legacy Canvas-level opt-in.
    #[serde(default)]
    pub focusable: bool,
    /// Nonnegative native Tab order; omission excludes this target from sequential navigation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab_index: Option<u32>,
}
/// Native target pointer/focus defaults require no redundant declarations.
fn enabled() -> bool {
    true
}

/// All native modifier states; unavailable physical key codes remain absent rather than guessed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasModifiers {
    /// Control modifier.
    pub ctrl: bool,
    /// Alt/Option modifier.
    pub alt: bool,
    /// Shift modifier.
    pub shift: bool,
    /// Windows/Command modifier.
    pub meta: bool,
    /// Fn modifier when supplied by the native platform.
    pub function: bool,
}
/// Separate native key transitions; text commits are not inferred from these events.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasKeyPhase {
    /// Native key pressed or repeated.
    Down,
    /// Native key released.
    Up,
}
/// One target-addressed input payload. The enclosing identity never comes from array order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CanvasInputKind {
    /// Native assistive action, allowed only by the named semantic node on this target.
    A11y {
        semantic: String,
        action: CanvasSemanticAction,
        value: Option<String>,
    },
    /// Recoverable native session failure; the guest may acknowledge pending text or replace the epoch.
    TextError {
        code: crate::api::ErrorCode,
        message: String,
    },
    /// Native system selection changed in the same pending draft as the text/IME stream.
    Selection {
        text_revision: u64,
        base_sequence: u64,
        range: CanvasTextRange,
    },
    /// Independent native text commit with an explicit predecessor and original-byte replacement.
    Text(CanvasTextEdit),
    /// Native IME preedit lifecycle, separate from domain edits and keys.
    Composition(CanvasComposition),
    /// Native focus transition for this exact target generation.
    Focus { focused: bool },
    /// Native pointer with explicit availability and capture cancellation.
    Pointer(CanvasPointerInput),
    /// Native continuous wheel observation, with original units.
    Wheel(CanvasWheelInput),
    /// Complete logical key event; unavailable physical/scancode information is explicitly None.
    Key {
        /// Press/release transition.
        phase: CanvasKeyPhase,
        /// Native logical key, at most 128 UTF-8 bytes.
        key: String,
        /// Platform physical key code when actually available.
        code: Option<String>,
        /// Native held-key flag; false on release.
        repeat: bool,
        /// All available native modifier states.
        modifiers: CanvasModifiers,
    },
}
/// Input from an actually painted target generation, before any subsequent guest redraw.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasInput {
    /// Scene identity actually painted.
    pub identity: u64,
    /// Scene drawing revision actually painted, independent of the outer UI revision.
    pub presented_revision: u64,
    /// Original target and nonzero epoch.
    pub target: String,
    /// Target generation actually receiving native input.
    pub epoch: u64,
    /// Strictly increasing host-native input order. Gaps also include input on other Canvas surfaces.
    pub sequence: u64,
    /// Original source authority, absent only on an unbound panel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<crate::api::DocumentVersion>,
    /// Original binary source authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<crate::api::FileVersion>,
    /// Projection at the actual native presentation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projection: Option<super::super::VisualViewport>,
    /// Bounded native payload.
    pub kind: CanvasInputKind,
}
/// Cleanup observations end ownership only; they never invoke a business or editor action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanvasCleanupKind {
    /// A previously focused native target blurred.
    Focus,
    /// A previously active native preedit was cancelled.
    Composition,
    /// A previously pressed/captured pointer was cancelled.
    Pointer,
}
impl CanvasInput {
    /// Classify a bounded cleanup observation before checking a retired target's exact native lease.
    /// A positive result grants no authority: Manager must match the old frame and consume its lease.
    pub fn cleanup_kind(&self) -> Option<CanvasCleanupKind> {
        if self.identity == 0
            || self.presented_revision == 0
            || self.epoch == 0
            || self.sequence == 0
            || self.target.is_empty()
            || self.target.len() > 128
            || self.target.chars().any(char::is_control)
        {
            return None;
        }
        match &self.kind {
            CanvasInputKind::Focus { focused: false } => Some(CanvasCleanupKind::Focus),
            CanvasInputKind::Composition(value)
                if value.phase == CanvasCompositionPhase::Cancel && value.valid(self.sequence) =>
            {
                Some(CanvasCleanupKind::Composition)
            }
            CanvasInputKind::Pointer(value)
                if value.phase == CanvasPointerPhase::Cancel && value.valid() =>
            {
                Some(CanvasCleanupKind::Pointer)
            }
            _ => None,
        }
    }
}
impl CanvasScene {
    /// Check `input` against this scene's current target generations, action declarations and payload limits.
    /// A true result is structural admission only: native adapters must also prove the shown frame,
    /// source/instance ownership and input order through the public Manager boundary. This predicate
    /// neither grants editor document access nor authorizes retired-target cleanup on its own.
    pub fn accepts_input(&self, input: &CanvasInput) -> bool {
        if input.identity != self.identity
            || input.presented_revision == 0
            || input.presented_revision > self.revision
            || input.sequence == 0
        {
            return false;
        }
        let Some(target) = self
            .targets
            .iter()
            .find(|target| target.id == input.target && target.epoch == input.epoch)
        else {
            return false;
        };
        match &input.kind {
            CanvasInputKind::A11y {
                semantic,
                action,
                value,
            } => self.semantics.iter().any(|node| {
                node.id == *semantic
                    && node.target.as_deref() == Some(&input.target)
                    && self.semantic_enabled(node)
                    && node.actions.contains(action)
                    && value.as_ref().is_none_or(|text| text.len() <= 65536)
                    && (*action == CanvasSemanticAction::SetValue || value.is_none())
            }),
            CanvasInputKind::TextError { code, message } => {
                target.text_input.is_some()
                    && matches!(
                        code,
                        crate::api::ErrorCode::Conflict
                            | crate::api::ErrorCode::LimitExceeded
                            | crate::api::ErrorCode::InvalidRequest
                            | crate::api::ErrorCode::OperationFailed
                    )
                    && !message.is_empty()
                    && message.len() <= 512
            }
            CanvasInputKind::Selection {
                text_revision,
                base_sequence,
                range,
            } => target.text_input.as_ref().is_some_and(|session| {
                *text_revision != 0
                    && *text_revision <= session.text_revision
                    && *base_sequence < input.sequence
                    && range.start <= range.end
                    && range.end <= 65536
            }),
            CanvasInputKind::Text(edit) => target.text_input.as_ref().is_some_and(|session| {
                !session.read_only
                    && edit.text_revision <= session.text_revision
                    && edit.valid(input.sequence)
            }),
            CanvasInputKind::Composition(composition) => {
                target.text_input.as_ref().is_some_and(|session| {
                    (!session.read_only
                        || matches!(
                            composition.phase,
                            CanvasCompositionPhase::Cancel | CanvasCompositionPhase::End
                        ))
                        && composition.text_revision <= session.text_revision
                        && composition.valid(input.sequence)
                })
            }
            CanvasInputKind::Focus { .. } => target.focusable,
            CanvasInputKind::Pointer(pointer) => target.pointer_events && pointer.valid(),
            CanvasInputKind::Wheel(wheel) => target.pointer_events && wheel.valid(),
            CanvasInputKind::Key {
                phase,
                key,
                code,
                repeat,
                ..
            } => {
                target.focusable
                    && !key.is_empty()
                    && key.len() <= 128
                    && !key.chars().any(char::is_control)
                    && code.as_ref().is_none_or(|code| {
                        !code.is_empty() && code.len() <= 128 && !code.chars().any(char::is_control)
                    })
                    && (*phase != CanvasKeyPhase::Up || !*repeat)
            }
        }
    }
}
