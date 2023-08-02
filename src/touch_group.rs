use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct TouchPos {
    pub cur_pos: (Option<i32>, Option<i32>),
    pub prev_pos: (Option<i32>, Option<i32>), // 第一次触摸没有prev
}

#[derive(Debug, Clone)]
pub struct TouchGroup {
    pub id_slot: HashMap<i32, Option<i32>>,
    pub slot_pos: HashMap<Option<i32>, TouchPos>,
}

impl TouchPos {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            cur_pos: (None, None),
            prev_pos: (None, None),
        }
    }

    pub fn x(&mut self, pos_x: i32) {
        self.prev_pos = self.cur_pos;
        self.cur_pos.0 = Some(pos_x);
    }

    pub fn y(&mut self, pos_y: i32) {
        self.prev_pos = self.cur_pos;
        self.cur_pos.1 = Some(pos_y);
    }
}

impl TouchGroup {
    #[must_use]
    pub fn new() -> Self {
        Self {
            id_slot: HashMap::new(),
            slot_pos: HashMap::new(),
        }
    }

    pub fn remove_id(&mut self) {
        let Some(id) = self.id_slot.keys().max().copied() else {
            return;
        };

        if let Some(slot) = self.id_slot.get(&id) {
            self.slot_pos.remove(slot);
        }
        self.id_slot.remove(&id);
    }
}
