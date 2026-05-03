use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use eyre::eyre;
use fixed_vec_deque::FixedVecDeque;
use parking_lot::{Condvar, Mutex};

use crate::hid::{Hid, HidDevice};

type Message = [u8; 64];

pub struct Device<'a> {
    hid_device: HidDevice<'a>,
    mailbox: Arc<Mailbox>,
    closed_receiver: mpsc::Receiver<()>,
}

impl<'a> Device<'a> {
    pub fn connect(hid: &'a Hid, vendor_id: u16, product_id: u16) -> eyre::Result<Device<'a>> {
        let device = hid.open(vendor_id, product_id)?;
        let read_device = device.reader();

        let (closed_sender, closed_receiver) = mpsc::sync_channel(1);

        let mailbox = Arc::new(Mailbox::new());

        thread::spawn({
            let mailbox = mailbox.clone();
            move || {
                while !mailbox.is_closed() {
                    let mut buf = [0u8; 64];

                    let Ok(res) = read_device.read_timeout(&mut buf, Duration::from_millis(100))
                    else {
                        break;
                    };

                    if res == 0 {
                        continue;
                    }

                    assert_eq!(res, 64);

                    if let Err(_) = mailbox.write(buf) {
                        break;
                    }
                }

                let _ = closed_sender.send(());
            }
        });

        Ok(Device {
            hid_device: device,
            mailbox,
            closed_receiver,
        })
    }

    #[expect(unused)]
    pub fn read(&self) -> eyre::Result<[u8; 64]> {
        self.mailbox.read().map_err(|_| eyre!("Mailbox is closed"))
    }

    pub fn read_timeout(&self, timeout: Duration) -> Result<[u8; 64], TimeoutError> {
        self.mailbox.read_timeout(timeout).map_err(|e| match e {
            WriteTimeoutError::Timeout => TimeoutError::Timeout,
            WriteTimeoutError::Closed => TimeoutError::Other(eyre!("Mailbox is closed")),
        })
    }

    pub fn write(&self, data: &[u8]) -> eyre::Result<()> {
        let size = self.hid_device.write(data)?;
        if size != data.len() {
            Err(eyre!("Only managed to write {size} bytes"))
        } else {
            Ok(())
        }
    }
}

pub enum TimeoutError {
    Timeout,
    Other(eyre::Report),
}

impl Drop for Device<'_> {
    fn drop(&mut self) {
        let _ = self.mailbox.close();
        let _ = self.closed_receiver.recv();
    }
}

struct Envelope(Message);

impl Default for Envelope {
    fn default() -> Self {
        Envelope([0; _])
    }
}

struct Mailbox {
    is_closed: AtomicBool,
    queue: Mutex<FixedVecDeque<[Envelope; 8]>>,
    condvar: Condvar,
}

enum WriteTimeoutError {
    Timeout,
    Closed,
}

impl Mailbox {
    fn new() -> Mailbox {
        Mailbox {
            is_closed: AtomicBool::new(false),
            queue: Mutex::new(FixedVecDeque::new()),
            condvar: Default::default(),
        }
    }

    fn is_closed(&self) -> bool {
        self.is_closed.load(Ordering::Acquire)
    }

    fn write(&self, msg: Message) -> Result<(), ()> {
        if self.is_closed() {
            return Err(());
        }
        *self.queue.lock().push_back() = Envelope(msg);
        self.condvar.notify_one();
        Ok(())
    }

    fn read(&self) -> Result<[u8; 64], ()> {
        let mut queue = self.queue.lock();
        loop {
            if self.is_closed() {
                return Err(());
            }
            if let Some(Envelope(msg)) = queue.pop_front() {
                return Ok(msg.clone());
            }
            self.condvar.wait(&mut queue);
        }
    }

    fn read_timeout(&self, timeout: Duration) -> Result<[u8; 64], WriteTimeoutError> {
        let mut queue = self.queue.lock();
        let deadline = Instant::now() + timeout;
        loop {
            if self.is_closed() {
                return Err(WriteTimeoutError::Closed);
            }

            if let Some(Envelope(msg)) = queue.pop_front() {
                return Ok(msg.clone());
            }

            let now = Instant::now();
            if now > deadline {
                return Err(WriteTimeoutError::Timeout);
            }

            self.condvar.wait_for(&mut queue, deadline - now);
        }
    }

    fn close(&self) {
        self.is_closed.store(true, Ordering::Release);
        self.condvar.notify_all();
    }
}
