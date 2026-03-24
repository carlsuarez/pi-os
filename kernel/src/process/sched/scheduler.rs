use crate::arch::IrqSpinLock;
use crate::process::pcb::Pid;
use alloc::collections::VecDeque;

const HIGH_QUANTA: usize = 3;
const MID_QUANTA: usize = 2;
const LOW_QUANTA: usize = 1;

pub struct Scheduler {
    inner: IrqSpinLock<SchedulerInner>,
}

struct SchedulerInner {
    // Realtime queue: ALWAYS runs first (strict priority)
    realtime_queue: VecDeque<Pid>,

    // Fair-share queues: Use 3:2:1 ratio
    high_queue: VecDeque<Pid>,
    mid_queue: VecDeque<Pid>,
    low_queue: VecDeque<Pid>,

    schedule_cycle: usize,
    time_slice: u32,
}

impl SchedulerInner {
    pub fn schedule(&mut self) -> Option<Pid> {
        // Realtime ALWAYS goes first (strict priority)
        if let Some(pid) = self.realtime_queue.pop_front() {
            return Some(pid);
        }

        // Then use ratio for other queues
        let step = self.schedule_cycle % (HIGH_QUANTA + MID_QUANTA + LOW_QUANTA);

        let queue: &mut VecDeque<Pid>;
        if step < HIGH_QUANTA {
            queue = &mut self.high_queue;
        } else if step < MID_QUANTA {
            queue = &mut self.mid_queue;
        } else {
            queue = &mut self.low_queue;
        }

        self.schedule_cycle += 1;

        queue.pop_front().or_else(|| self.fallback())
    }

    fn fallback(&self) -> Option<Pid> {
        None
    }
}
