pub(crate) mod analyze;
mod read;
pub(crate) mod touch_group;

use std::{
    collections::HashMap,
    error::Error,
    fs,
    ops::Deref,
    sync::{
        atomic::AtomicUsize,
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
/// let listener = TouchListener::new(5).unwrap();
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
    min_pixel: Arc<AtomicUsize>,
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
    /// min_pixel: the minimum pixel point judged as sliding, recommand 5
    ///
    /// # Errors
    ///
    /// No touch device
    ///
    /// Failed to create thread
    pub fn new(min_pixel: usize) -> Result<Self, Box<dyn Error>> {
        let devices: Vec<_> = fs::read_dir("/dev/input")?
            .filter_map(|f| {
                let f = f.ok()?;
                let path = f.path();
                let device = Device::open(path).ok()?;

                let event_len = "event".len();
                let id: usize = f.file_name().into_string().ok()?[event_len..]
                    .trim()
                    .parse()
                    .ok()?;

                Some((id, device))
            })
            .filter(|(_, d)| d.supported_events().contains(EventType::ABSOLUTE))
            .collect();

        if devices.is_empty() {
            return Err("No usable touch device".into());
        }

        let mut status_map = HashMap::new();
        let (sx, rx) = mpsc::sync_channel(1);
        let min_pixel = Arc::new(AtomicUsize::new(min_pixel));

        for (id, device) in devices {
            let touch_status = Arc::new(Atomic::new(TouchStatus::None));
            let touch_status_clone = touch_status.clone();
            let sx = sx.clone();
            let min_pixel = min_pixel.clone();

            status_map.insert(id, touch_status);

            thread::Builder::new()
                .name("TouchDeviceListener".into())
                .spawn(move || read::daemon_thread(device, &touch_status_clone, &sx, &min_pixel))?;
        }

        if status_map.is_empty() {
            return Err("No usable touch device".into());
        }

        Ok(Self {
            status_map,
            wait: rx,
            min_pixel,
        })
    }

    /// Set the minimum pixel point judged as sliding
    pub fn min_pixel(&self, p: usize) {
        self.min_pixel.store(p, Ordering::Release);
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
