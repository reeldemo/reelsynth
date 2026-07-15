//! Undo/redo stack for compose note edits.

use reelsynth::MidiNote;

#[derive(Clone, Debug, PartialEq)]
pub enum ComposeCommand {
    AddNote {
        track: usize,
        clip: usize,
        note: MidiNote,
    },
    DeleteNotes {
        track: usize,
        clip: usize,
        notes: Vec<(usize, MidiNote)>,
    },
    MoveNotes {
        track: usize,
        clip: usize,
        entries: Vec<(usize, f32, f32, u8)>,
        delta_beats: f32,
        delta_pitch: i8,
    },
    ResizeNotes {
        track: usize,
        clip: usize,
        entries: Vec<(usize, f32, f32, f32, f32)>,
    },
    AddNotes {
        track: usize,
        clip: usize,
        notes: Vec<MidiNote>,
    },
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct CommandHistory {
    undo: Vec<ComposeCommand>,
    redo: Vec<ComposeCommand>,
    max_depth: usize,
}

impl CommandHistory {
    pub fn new(max_depth: usize) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            max_depth,
        }
    }

    pub fn push(&mut self, cmd: ComposeCommand) {
        self.redo.clear();
        self.undo.push(cmd);
        while self.undo.len() > self.max_depth {
            self.undo.remove(0);
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn take_undo(&mut self) -> Option<ComposeCommand> {
        self.undo.pop().inspect(|c| self.redo.push(c.clone()))
    }

    pub fn take_redo(&mut self) -> Option<ComposeCommand> {
        self.redo.pop().inspect(|c| self.undo.push(c.clone()))
    }
}
