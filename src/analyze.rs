use std::sync::{mpsc::SyncSender, Arc};

use atomic::Ordering;

use super::{
    touch_group::{TouchGroup, TouchPos},
    AtomicTouchStatus, TouchStatus,
};

const MIN_SLIDE_PIXELS: f64 = 10.0;

pub fn analyze(group: &TouchGroup, status: &Arc<AtomicTouchStatus>, notice: &SyncSender<()>) {
    let new_status;

    if group.slot_pos.values().any(on_slide) {
        new_status = TouchStatus::Slide;
    } else if group.slot_pos.is_empty() {
        new_status = TouchStatus::None;
    } else {
        new_status = TouchStatus::Click;
    }

    if status.load(Ordering::Acquire) != new_status {
        status.store(new_status, Ordering::Release);
        let _ = notice.try_send(());
    }
}

fn on_slide(pos: &TouchPos) -> bool {
    let (Some(cur_x), Some(cur_y)) = pos.cur_pos else {
        return false;
    };

    let (Some(prev_x), Some(prev_y)) = pos.prev_pos else {
        return false;
    };

    let len_x = (cur_x - prev_x).abs();
    let len_y = (cur_y - prev_y).abs();

    let len = f64::from(len_x.pow(2) + len_y.pow(2)).sqrt();

    len >= MIN_SLIDE_PIXELS
}
