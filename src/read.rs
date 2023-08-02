use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc::SyncSender,
    Arc,
};

use evdev::{AbsoluteAxisType, Device, EventType, InputEventKind};

use super::{
    analyze::analyze,
    touch_group::{TouchGroup, TouchPos},
    AtomicTouchStatus,
};

pub fn daemon_thread(
    mut touch_device: Device,
    status: &Arc<AtomicTouchStatus>,
    notice: &SyncSender<()>,
    min_pixel: &Arc<AtomicUsize>,
) {
    if !touch_device
        .supported_events()
        .contains(EventType::ABSOLUTE)
    {
        eprintln!("{:?} is not an touch device!", touch_device.name().unwrap());
        return;
    }

    let mut group = TouchGroup::new();
    let mut target = (None, None); // id, slot
    let mut cache = Vec::new();

    loop {
        let events = touch_device.fetch_events().unwrap();

        for event in events {
            if let InputEventKind::AbsAxis(abs) = event.kind() {
                match abs {
                    AbsoluteAxisType::ABS_MT_TRACKING_ID => {
                        update_group(
                            &mut group,
                            &mut target,
                            &mut cache,
                            status,
                            notice,
                            min_pixel,
                        );
                        target.0 = Some(event.value());
                    }
                    AbsoluteAxisType::ABS_MT_SLOT => {
                        update_group(
                            &mut group,
                            &mut target,
                            &mut cache,
                            status,
                            notice,
                            min_pixel,
                        );
                        target.1 = Some(event.value());
                    }
                    AbsoluteAxisType::ABS_MT_POSITION_X | AbsoluteAxisType::ABS_MT_POSITION_Y => {
                        cache.push((abs, event.value()));
                    }
                    _ => (),
                }
            } else if let InputEventKind::Synchronization(_) = event.kind() {
                update_group(
                    &mut group,
                    &mut target,
                    &mut cache,
                    status,
                    notice,
                    min_pixel,
                );
            }
        }
    }
}

fn update_group(
    group: &mut TouchGroup,
    target: &mut (Option<i32>, Option<i32>),
    events: &mut Vec<(AbsoluteAxisType, i32)>,
    status: &Arc<AtomicTouchStatus>,
    notice: &SyncSender<()>,
    min_pixel: &Arc<AtomicUsize>,
) {
    if events.is_empty() && target.0.is_none() {
        return;
    } // 如果没有事件，也没有更新/删除id的目标，那么就没有任何事要做

    if let Some(id) = target.0 {
        use std::collections::hash_map::Entry;

        if id == -1 {
            group.remove_id();
            target.0 = None;
            analyze(group, status, notice, min_pixel.load(Ordering::Acquire));
            return;
        }

        if let Entry::Vacant(e) = group.id_slot.entry(id) {
            e.insert(target.1);
            group.slot_pos.insert(target.1, TouchPos::new());
        }
    }

    analyze(group, status, notice, min_pixel.load(Ordering::Acquire));

    for (t, v) in &*events {
        analyze(group, status, notice, min_pixel.load(Ordering::Acquire));

        let Some(pos) = group.slot_pos.get_mut(&target.1) else {
            *target = (None, None);
            return;
        };

        match *t {
            AbsoluteAxisType::ABS_MT_POSITION_X => pos.x(*v),
            AbsoluteAxisType::ABS_MT_POSITION_Y => pos.y(*v),
            _ => (),
        }
    }

    events.clear();
    *target = (None, None);
}
