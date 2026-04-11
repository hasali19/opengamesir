use std::thread;
use std::time::Duration;

use eyre::eyre;

use crate::hid::{Hid, HidDevice};

pub struct Device<'a> {
    hid_device: HidDevice<'a>,
    read_receiver: kanal::Receiver<[u8; 64]>,
    closed_receiver: kanal::Receiver<()>,
}

impl<'a> Device<'a> {
    pub fn connect(hid: &'a Hid, vendor_id: u16, product_id: u16) -> eyre::Result<Device<'a>> {
        let device = hid.open(vendor_id, product_id)?;
        let read_device = device.reader();

        let (read_sender, read_receiver) = kanal::unbounded();
        let (closed_sender, closed_receiver) = kanal::bounded(1);

        thread::spawn(move || {
            while !read_sender.is_closed() {
                let mut buf = [0u8; 64];

                let Ok(res) = read_device.read_timeout(&mut buf, Duration::from_millis(100)) else {
                    break;
                };

                if res == 0 {
                    continue;
                }

                assert_eq!(res, 64);

                if buf[0] == 18 {
                    // TODO: Handle state messages
                    continue;
                }

                if let Err(_) = read_sender.send(buf) {
                    break;
                }
            }

            let _ = closed_sender.send(());
        });

        Ok(Device {
            hid_device: device,
            read_receiver,
            closed_receiver,
        })
    }

    #[expect(unused)]
    pub fn read(&self) -> eyre::Result<[u8; 64]> {
        self.read_receiver.recv().map_err(Into::into)
    }

    pub fn read_timeout(&self, timeout: Duration) -> Result<[u8; 64], TimeoutError> {
        self.read_receiver.recv_timeout(timeout).map_err(|e| {
            if let kanal::ReceiveErrorTimeout::Timeout = e {
                TimeoutError::Timeout
            } else {
                TimeoutError::Other(e.into())
            }
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
        let _ = self.read_receiver.close();
        let _ = self.closed_receiver.recv();
    }
}
