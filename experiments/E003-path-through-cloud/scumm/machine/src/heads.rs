//! model.py `HEADS` for ops and starts: each statement name's effect, and the text model.py keeps for an
//! unmodelled one. Jumps and assignments are dispatched before the table is read (model.py `_step`), so
//! only names of `Op` and `Start` statements are here. A name the table lacks is "unmodelled", "no rule".

use scumm_ir::{known, Sym};

/// A statement's effect (model.py's HEADS effects; "control" is a jump's, dispatched apart).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Effect {
    Write,
    Yield,
    Wait,
    End,
    None,
    Deferred,
    Unmodelled(&'static str),
}

/// The effect of an op or start named `name` (model.py `HEADS.get(head, ("unmodelled", "", "no rule"))`).
pub fn effect(name: Sym) -> Effect {
    effect_with(name, false)
}

/// `effect`, with E014 amendment (8)'s switch `FMI_READ_SOUND` (`Machine::sound_ops`): the ops whose only effect in
/// ScummVM is on the sound or the screen have no tracked effect, as startSound and stopSound have none:
/// soundKludge (script_v5.cpp o5_soundKludge: the sound engine's command queue; its answer reaches the scripts only
/// through VAR_SOUNDRESULT, which the engine writes outside them, sound.cpp processSoundQueues), setRoomShadow
/// (o5_roomOps: the palette's shadow table) and debug (o5_debug: a debug print). Off, each has no rule and ends its
/// path "unmodelled", which the reader drops: every path through a sound command is lost.
pub fn effect_with(name: Sym, sound: bool) -> Effect {
    use Effect::*;
    match name {
        known::SOUND_KLUDGE | known::SET_ROOM_SHADOW | known::DEBUG if sound => None,
        known::BREAK_HERE | known::DELAY | known::DELAY_VARIABLE => Yield,
        known::WAIT_FOR_ACTOR | known::WAIT_FOR_MESSAGE | known::WAIT_FOR_CAMERA | known::WAIT_FOR_SENTENCE => Wait,
        known::STOP_OBJECT_CODE | known::CHAIN_SCRIPT => End,
        known::DO_SENTENCE => Deferred,
        known::START_SCRIPT
        | known::START_OBJECT
        | known::STOP_SCRIPT
        | known::STOP_OBJECT_SCRIPT
        | known::SET_STATE
        | known::SET_CLASS
        | known::SET_OWNER_OF
        | known::PICKUP_OBJECT
        | known::SET_OBJECT_NAME
        | known::DRAW_OBJECT
        | known::VERB_OPS
        | known::SAVE_VERBS
        | known::RESTORE_VERBS
        | known::SET_VAR_RANGE
        | known::LOAD_ROOM
        | known::LOAD_ROOM_WITH_EGO
        | known::ACTOR_FOLLOW_CAMERA
        | known::SET_CAMERA_AT
        | known::CUTSCENE
        | known::END_CUTSCENE
        | known::BEGIN_OVERRIDE
        | known::END_OVERRIDE
        | known::USERPUT_ON
        | known::USERPUT_OFF
        | known::USERPUT_SOFT_ON
        | known::USERPUT_SOFT_OFF
        | known::CURSOR_SHOW
        | known::CURSOR_HIDE
        | known::CURSOR_SOFT_ON
        | known::CURSOR_SOFT_OFF
        | known::ROOM_SCROLL
        | known::PRINT
        | known::FREEZE_SCRIPTS => Write,
        known::INIT_CHARSET
        | known::CURSOR_COMMAND
        | known::PRINT_EGO
        | known::ANIMATE_COSTUME
        | known::ACTOR_OPS
        | known::PUT_ACTOR
        | known::PUT_ACTOR_IN_ROOM
        | known::PUT_ACTOR_AT_OBJECT
        | known::WALK_ACTOR_TO
        | known::WALK_ACTOR_TO_OBJECT
        | known::WALK_ACTOR_TO_ACTOR
        | known::FACE_ACTOR
        | known::SET_BOX_FLAGS
        | known::CREATE_BOX_MATRIX
        | known::START_SOUND
        | known::STOP_SOUND
        | known::SET_STRING_CHAR
        | known::PUT_CODE_IN_STRING
        | known::COPY_STRING
        | known::CREATE_STRING
        | known::SET_SCREEN
        | known::SET_PAL_COLOR
        | known::SHAKE_ON
        | known::SHAKE_OFF
        | known::ROOM_INTENSITY
        | known::SET_RGB_ROOM_INTENSITY
        | known::SCREEN_EFFECT
        | known::DRAW_BOX
        | known::PSEUDO_ROOM
        | known::PAN_CAMERA_TO
        | known::RESOURCE_LOAD_SCRIPT
        | known::RESOURCE_LOCK_SCRIPT
        | known::RESOURCE_UNLOCK_SCRIPT
        | known::RESOURCE_LOAD_SOUND
        | known::RESOURCE_LOCK_SOUND
        | known::RESOURCE_UNLOCK_SOUND
        | known::RESOURCE_LOAD_COSTUME
        | known::RESOURCE_LOCK_COSTUME
        | known::RESOURCE_UNLOCK_COSTUME
        | known::RESOURCE_LOAD_ROOM
        | known::RESOURCE_LOCK_ROOM
        | known::RESOURCE_UNLOCK_ROOM
        | known::RESOURCE_LOAD_CHARSET
        | known::RESOURCE_CLEAR_HEAP => None,
        known::SYSTEM_OPS => Unmodelled("restart, pause or quit"),
        _ => Unmodelled("no rule"),
    }
}
