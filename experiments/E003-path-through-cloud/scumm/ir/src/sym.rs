//! Interned strings. Every name and text the IR holds (op names, getter names, string literals,
//! statement texts, file names) is a [`Sym`], a `u32` index into one [`Interner`].
//!
//! The interner starts with the names the analyses and the machine dispatch on, in a fixed order,
//! so each has a constant `Sym` in [`known`]: `known::BREAK_HERE`, `known::GET_RANDOM_NR`, ... A
//! match on an op name is then an integer match.

use rustc_hash::FxHashMap;

/// An interned string: an index into [`Interner`].
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Default)]
pub struct Sym(pub u32);

/// Owns every string; [`Sym`]s index it.
#[derive(Clone, Debug)]
pub struct Interner {
    strs: Vec<Box<str>>,
    map: FxHashMap<Box<str>, Sym>,
}

impl Default for Interner {
    fn default() -> Self {
        Self::new()
    }
}

impl Interner {
    /// An interner holding the [`known`] names at their constant ids.
    pub fn new() -> Self {
        let mut i = Interner { strs: Vec::with_capacity(1 << 16), map: FxHashMap::default() };
        for (k, s) in known::ALL.iter().enumerate() {
            let got = i.intern(s);
            debug_assert_eq!(got.0 as usize, k);
        }
        i
    }

    pub fn intern(&mut self, s: &str) -> Sym {
        if let Some(&y) = self.map.get(s) {
            return y;
        }
        let y = Sym(self.strs.len() as u32);
        self.strs.push(s.into());
        self.map.insert(s.into(), y);
        y
    }

    /// The symbol of `s` if it was interned.
    pub fn get(&self, s: &str) -> Option<Sym> {
        self.map.get(s).copied()
    }

    #[inline]
    pub fn str(&self, y: Sym) -> &str {
        &self.strs[y.0 as usize]
    }

    pub fn len(&self) -> usize {
        self.strs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strs.is_empty()
    }
}

macro_rules! known {
    ($($id:ident = $s:literal),* $(,)?) => {
        /// The names with constant symbols, interned first by [`Interner::new`].
        pub mod known {
            use super::Sym;
            /// Every known name, in id order.
            pub const ALL: &[&str] = &[$($s),*];
            #[allow(non_camel_case_types, clippy::upper_case_acronyms, dead_code)]
            #[repr(u32)]
            enum Id { $($id),* }
            $(pub const $id: Sym = Sym(Id::$id as u32);)*
        }
    };
}

known! {
    EMPTY = "",
    // flags descumm prints as bare words
    F = "F", R = "R", STOP = "STOP",
    // yields and waits
    BREAK_HERE = "breakHere", DELAY = "delay", DELAY_VARIABLE = "delayVariable",
    WAIT_FOR_ACTOR = "WaitForActor", WAIT_FOR_MESSAGE = "WaitForMessage", WAIT_FOR_CAMERA = "WaitForCamera",
    WAIT_FOR_SENTENCE = "WaitForSentence",
    // ends and starts
    STOP_OBJECT_CODE = "stopObjectCode", CHAIN_SCRIPT = "chainScript", START_SCRIPT = "startScript",
    START_OBJECT = "startObject", STOP_SCRIPT = "stopScript", STOP_OBJECT_SCRIPT = "stopObjectScript",
    DO_SENTENCE = "doSentence",
    // fact writers
    SET_STATE = "setState", SET_CLASS = "setClass", SET_OWNER_OF = "setOwnerOf", PICKUP_OBJECT = "pickupObject",
    SET_OBJECT_NAME = "setObjectName", DRAW_OBJECT = "drawObject", VERB_OPS = "VerbOps", SAVE_VERBS = "saveVerbs",
    RESTORE_VERBS = "restoreVerbs", SET_VAR_RANGE = "setVarRange", LOAD_ROOM = "loadRoom",
    LOAD_ROOM_WITH_EGO = "loadRoomWithEgo", ACTOR_FOLLOW_CAMERA = "actorFollowCamera", SET_CAMERA_AT = "setCameraAt",
    CUTSCENE = "cutscene", END_CUTSCENE = "endCutscene", BEGIN_OVERRIDE = "beginOverride",
    END_OVERRIDE = "endOverride", USERPUT_ON = "UserputOn", USERPUT_OFF = "UserputOff",
    USERPUT_SOFT_ON = "UserputSoftOn", USERPUT_SOFT_OFF = "UserputSoftOff", CURSOR_SHOW = "CursorShow",
    CURSOR_HIDE = "CursorHide", CURSOR_SOFT_ON = "CursorSoftOn", CURSOR_SOFT_OFF = "CursorSoftOff",
    ROOM_SCROLL = "RoomScroll", INIT_CHARSET = "InitCharset", CURSOR_COMMAND = "CursorCommand", PRINT = "print",
    PRINT_EGO = "printEgo", FREEZE_SCRIPTS = "freezeScripts", PSEUDO_ROOM = "PseudoRoom",
    PAN_CAMERA_TO = "panCameraTo", SYSTEM_OPS = "systemOps",
    // ops with no tracked effect (model.HEADS "none")
    ANIMATE_COSTUME = "animateCostume", ACTOR_OPS = "ActorOps", PUT_ACTOR = "putActor",
    PUT_ACTOR_IN_ROOM = "putActorInRoom", PUT_ACTOR_AT_OBJECT = "putActorAtObject", WALK_ACTOR_TO = "walkActorTo",
    WALK_ACTOR_TO_OBJECT = "walkActorToObject", WALK_ACTOR_TO_ACTOR = "walkActorToActor", FACE_ACTOR = "faceActor",
    SET_BOX_FLAGS = "setBoxFlags", CREATE_BOX_MATRIX = "createBoxMatrix", START_SOUND = "startSound",
    STOP_SOUND = "stopSound", SET_STRING_CHAR = "SetStringChar", PUT_CODE_IN_STRING = "PutCodeInString",
    COPY_STRING = "CopyString", CREATE_STRING = "CreateString", SET_SCREEN = "SetScreen",
    SET_PAL_COLOR = "SetPalColor", SHAKE_ON = "ShakeOn", SHAKE_OFF = "ShakeOff", ROOM_INTENSITY = "RoomIntensity",
    SET_RGB_ROOM_INTENSITY = "setRGBRoomIntensity", SCREEN_EFFECT = "screenEffect", DRAW_BOX = "drawBox",
    RESOURCE_LOAD_SCRIPT = "Resource.loadScript", RESOURCE_LOCK_SCRIPT = "Resource.lockScript",
    RESOURCE_UNLOCK_SCRIPT = "Resource.unlockScript", RESOURCE_LOAD_SOUND = "Resource.loadSound",
    RESOURCE_LOCK_SOUND = "Resource.lockSound", RESOURCE_UNLOCK_SOUND = "Resource.unlockSound",
    RESOURCE_LOAD_COSTUME = "Resource.loadCostume", RESOURCE_LOCK_COSTUME = "Resource.lockCostume",
    RESOURCE_UNLOCK_COSTUME = "Resource.unlockCostume", RESOURCE_LOAD_ROOM = "Resource.loadRoom",
    RESOURCE_LOCK_ROOM = "Resource.lockRoom", RESOURCE_UNLOCK_ROOM = "Resource.unlockRoom",
    RESOURCE_LOAD_CHARSET = "Resource.loadCharset", RESOURCE_CLEAR_HEAP = "Resource.clearHeap",
    // getters (model.GETTERS)
    CLASS_OF_IS = "classOfIs", GET_OBJECT_OWNER = "getObjectOwner", GET_OBJECT_STATE = "getObjectState",
    IS_SCRIPT_RUNNING = "isScriptRunning", GET_INVENTORY_COUNT = "getInventoryCount",
    GET_VERB_ENTRY_POINT = "getVerbEntryPoint", GET_RANDOM_NR = "getRandomNr", GET_ACTOR_X = "getActorX",
    GET_ACTOR_Y = "getActorY", GET_ACTOR_WALK_BOX = "getActorWalkBox", GET_ACTOR_ELEVATION = "getActorElevation",
    GET_DIST = "getDist", FIND_OBJECT = "findObject", ACTOR_FROM_POS = "actorFromPos",
    GET_ACTOR_ROOM = "getActorRoom", GET_ACTOR_COSTUME = "getActorCostume", GET_ACTOR_FACING = "getActorFacing",
    GET_ACTOR_MOVING = "getActorMoving", GET_ACTOR_WIDTH = "getActorWidth", IS_SOUND_RUNNING = "isSoundRunning",
    GET_STRING_CHAR = "GetStringChar", FIND_INVENTORY = "findInventory",
    // text codes inside strings
    GET_INT = "getInt", GET_NAME = "getName", GET_STRING = "getString", GET_VERB = "getVerb",
    KEEP_TEXT = "keepText", NEWLINE = "newline", UNKNOWN8 = "unknown8", WAIT = "wait",
    // sub-ops
    NEW = "New", ON = "On", OFF = "Off", DIM = "Dim", DELETE = "Delete", SET_XY = "setXY", SET_IMAGE = "setImage",
    TEXT = "Text",
}
