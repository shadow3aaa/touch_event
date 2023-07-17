pub(crate) mod analyze;
mod read;
pub(crate) mod touch_group;

use std::{
    collections::HashMap,
    error::Error,
    fs,
    ops::Deref,
    sync::{
        mpsc::{self, Receiver},
        Arc,
    },
    thread,
    time::Duration,
};

use atomic::{Atomic, Ordering};
use evdev::{Device, EventType};

/// Listen for touch events
///
/// Implemented[`std::ops::Deref`]to access internal`status_map`
///
/// # Example
///
/// ```ignore
/// use std::{thread, time::Duration, sync::atomic::Ordering};
/// use touch_event::TouchListener;
///
/// let listener = TouchListener::new().unwrap();
/// thread::sleep(Duration::from_secs(1)); // Just listen for a while
///
/// // Deref to HashMap inside it
/// for atom_status in listener.values() {
///     let status = atom_status.load(Ordering::Acquire);
///     println!("{status:?}");
/// }
/// ```
///
/// note: This is untested when I was documenting, because my compiling environment does not have a touch screen and cannot test this.But it has been tested on other devices

#[derive(Debug)]
pub struct TouchListener {
    status_map: HashMap<usize, Arc<AtomicTouchStatus>>,
    wait: Receiver<()>,
}

pub(crate) type AtomicTouchStatus = Atomic<TouchStatus>;

/// Indicates the current touch state
///
/// [`TouchStatus::Slide`]: At least one touch point is sliding. Of course, this also means that there is at least one touch point on the screen
///
/// [`TouchStatus::Click`]: There are touch points on the screen, but no touch points are sliding
///
/// [`TouchStatus::None`]: There are no touch points
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TouchStatus {
    Slide,
    Click,
    None,
}

impl Deref for TouchListener {
    type Target = HashMap<usize, Arc<AtomicTouchStatus>>;

    fn deref(&self) -> &Self::Target {
        &self.status_map
    }
}

impl TouchListener {
    /// Allocate a listening thread for each touch device, and construct [`TouchListener`]
    ///
    /// # Errors
    ///
    /// No touch device / Failed to create thread
    ///
    /// # Panics
    ///
    /// `/dev/input` Does not exist / Failed to open device
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let devices = fs::read_dir("/dev/input")?
            .map(|f| {
                let f = f.unwrap();
                let path = f.path();
                let device = Device::open(path).unwrap();

                let event_len = "event".len();
                let id: usize = f.file_name().into_string().unwrap()[event_len..]
                    .trim()
                    .parse()
                    .unwrap();
                (id, device)
            })
            .filter(|(_, d)| d.supported_events().contains(EventType::ABSOLUTE));

        let mut status_map = HashMap::new();
        let (sx, rx) = mpsc::sync_channel(1);

        for (id, device) in devices {
            let touch_status = Arc::new(Atomic::new(TouchStatus::None));
            let touch_status_clone = touch_status.clone();
            let sx = sx.clone();

            status_map.insert(id, touch_status);

            thread::Builder::new()
                .name("TouchDeviceListener".into())
                .spawn(move || read::daemon_thread(device, &touch_status_clone, &sx))?;
        }

        if status_map.is_empty() {
            return Err("No touch device".into());
        }

        Ok(Self {
            status_map,
            wait: rx,
        })
    }

    /// Block and waiting for touch status to update
    ///
    /// # Errors
    ///
    /// Monitor threads had paniced
    #[inline]
    pub fn wait(&self) -> Result<(), &'static str> {
        self.wait.recv().map_err(|_| "Monitor threads had paniced")
    }

    /// Block and waiting for touch status to update, unless timeout
    ///
    /// # Errors
    ///
    /// Monitor threads had paniced
    #[inline]
    pub fn wait_timeout(&self, t: Duration) -> Result<(), &'static str> {
        self.wait
            .recv_timeout(t)
            .map_err(|_| "Monitor threads had paniced")
    }

    /// Analyze the status of all current devices and return a comprehensive status.
    ///
    /// [`TouchStatus::Slide`] state, [`TouchStatus::Click`] state, [`TouchStatus::None`] state
    ///
    /// If at least one device is in the corresponding state, then the corresponding state is true
    pub fn status(&self) -> (bool, bool, bool) {
        let slide = self
            .status_map
            .values()
            .any(|s| s.load(Ordering::Acquire) == TouchStatus::Slide);
        let click = self
            .status_map
            .values()
            .any(|s| s.load(Ordering::Acquire) == TouchStatus::Click);
        let none = self
            .status_map
            .values()
            .any(|s| s.load(Ordering::Acquire) == TouchStatus::None);

        (slide, click, none)
    }
}
