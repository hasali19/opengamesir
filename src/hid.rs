use std::marker::PhantomData;
use std::ptr::{NonNull, null};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use eyre::{bail, eyre};
use hidapi_sys::{
    hid_close, hid_device, hid_error, hid_exit, hid_init, hid_open, hid_read, hid_read_error,
    hid_read_timeout, hid_write,
};
use widestring::U32CStr;

static IS_INIT: AtomicBool = AtomicBool::new(false);

pub struct Hid {
    // Most hid functions are not safe to use from different threads.
    _not_send: PhantomData<*const ()>,
}

impl Hid {
    pub fn new() -> eyre::Result<Hid> {
        if IS_INIT.swap(true, Ordering::AcqRel) {
            bail!("Multiple instances of Hid are not allowed");
        }

        if unsafe { hid_init() } < 0 {
            bail!("failed to initialise hidapi");
        }

        Ok(Hid {
            _not_send: PhantomData,
        })
    }

    pub fn open<'a>(&'a self, vendor_id: u16, product_id: u16) -> eyre::Result<HidDevice<'a>> {
        let device = unsafe { hid_open(vendor_id, product_id, null()) };

        if device.is_null() {
            return Err(eyre!(get_error(device)));
        }

        Ok(HidDevice {
            _hid: PhantomData,
            device,
            reader_count: Box::leak(Box::new(AtomicUsize::new(0))).into(),
        })
    }
}

impl Drop for Hid {
    fn drop(&mut self) {
        if unsafe { hid_exit() } != 0 {
            panic!("Failed to shutdown hid");
        }
        IS_INIT.store(false, Ordering::Release);
    }
}

pub struct HidDevice<'a> {
    _hid: PhantomData<&'a Hid>,
    device: *mut hid_device,
    reader_count: NonNull<AtomicUsize>,
}

impl<'a> HidDevice<'a> {
    pub fn read(&self, buf: &mut [u8]) -> eyre::Result<usize> {
        let res = unsafe { hid_read(self.device, buf.as_mut_ptr(), buf.len()) };
        self.check_error(res)
    }

    pub fn write(&self, data: &[u8]) -> eyre::Result<usize> {
        let res = unsafe { hid_write(self.device, data.as_ptr(), data.len()) };
        self.check_error(res)
    }

    /// Returns a `HidReadDevice` that can be safely used to read from another
    /// thread.
    ///
    /// All readers must be dropped before dropping the parent `HidDevice`.
    pub fn reader(&self) -> HidReadDevice {
        let reader_count = unsafe { &*self.reader_count.as_ptr() };
        reader_count.fetch_add(1, Ordering::AcqRel);
        HidReadDevice {
            device: self.device,
            reader_count: self.reader_count,
        }
    }

    fn check_error(&self, res: i32) -> eyre::Result<usize> {
        if res == -1 {
            Err(eyre!(get_error(self.device)))
        } else {
            Ok(res as usize)
        }
    }
}

impl Drop for HidDevice<'_> {
    fn drop(&mut self) {
        let reader_count = unsafe { &*self.reader_count.as_ptr() };
        if reader_count.load(Ordering::Acquire) > 0 {
            panic!("HidDevice cannot be dropped while there are active HidReadDevices");
        }
        unsafe { hid_close(self.device) };
    }
}

fn get_error(device: *mut hid_device) -> String {
    let error = unsafe { hid_error(device) };
    let error = unsafe { U32CStr::from_ptr_str(error.cast()) };
    error.to_string_lossy()
}

pub struct HidReadDevice {
    device: *mut hid_device,
    reader_count: NonNull<AtomicUsize>,
}

unsafe impl Send for HidReadDevice {}

impl HidReadDevice {
    pub fn read(&self, buf: &mut [u8]) -> eyre::Result<usize> {
        let res = unsafe { hid_read(self.device, buf.as_mut_ptr(), buf.len()) };
        self.check_error(res)
    }

    pub fn read_timeout(&self, buf: &mut [u8], timeout: Duration) -> eyre::Result<usize> {
        let res = unsafe {
            hid_read_timeout(
                self.device,
                buf.as_mut_ptr(),
                buf.len(),
                timeout.as_millis().try_into()?,
            )
        };
        self.check_error(res)
    }

    fn check_error(&self, res: i32) -> eyre::Result<usize> {
        if res == -1 {
            Err(eyre!(get_read_error(self.device)))
        } else {
            Ok(res as usize)
        }
    }
}

impl Drop for HidReadDevice {
    fn drop(&mut self) {
        unsafe { &*self.reader_count.as_ptr() }.fetch_sub(1, Ordering::AcqRel);
    }
}

fn get_read_error(device: *mut hid_device) -> String {
    let error = unsafe { hid_read_error(device) };
    let error = unsafe { U32CStr::from_ptr_str(error.cast()) };
    error.to_string_lossy()
}
