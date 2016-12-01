
#![feature(integer_atomics)]

extern crate uuid;
extern crate email_format;
extern crate resolv;

#[cfg(test)]
mod tests;
mod worker;
pub mod error;
mod email;
pub mod storage;

use std::sync::{mpsc, Arc, RwLock};
use std::sync::atomic::{AtomicU8, Ordering};
use std::thread;
use std::ops::Drop;
use email_format::Email as RfcEmail;

use worker::{Worker, Message};
use error::Error;
use email::Email;
use storage::MailstromStorage;

pub use worker::WorkerStatus;

pub struct Config
{
    pub helo_name: String
}

pub struct Mailstrom<S: MailstromStorage + 'static>
{
    config: Config,
    sender: mpsc::Sender<Message>,
    worker_status: Arc<AtomicU8>,
    storage: Arc<RwLock<S>>,
}

impl<S: MailstromStorage + 'static> Mailstrom<S>
{
    /// Create a new Mailstrom instance for sending emails.
    pub fn new(config: Config, storage: S) -> Mailstrom<S>
    {
        let (sender, receiver) = mpsc::channel();

        let storage = Arc::new(RwLock::new(storage));

        let worker_status = Arc::new(AtomicU8::new(WorkerStatus::Ok as u8));

        let mut worker = Worker::new(receiver, storage.clone(), worker_status.clone(), &*config.helo_name);

        let _ = thread::spawn(move|| {
            worker.run();
        });

        Mailstrom {
            config: config,
            sender: sender,
            worker_status: worker_status,
            storage: storage,
        }
    }

    /// Ask Mailstrom to die.  This is not required, you can simply let it fall out
    /// of scope and it will clean itself up.
    pub fn die(&mut self) -> Result<(), Error>
    {
        try!(self.sender.send(Message::Terminate));
        Ok(())
    }

    /// Determine the status of the worker
    pub fn worker_status(&self) -> WorkerStatus
    {
        WorkerStatus::from_u8(self.worker_status.load(Ordering::SeqCst))
    }

    /// Send an email
    pub fn send_email(&mut self, rfc_email: RfcEmail) -> Result<(), Error>
    {
        let email = try!(Email::from_rfc_email(rfc_email, &*self.config.helo_name));

        Ok(try!(self.sender.send(Message::SendEmail(email))))
    }
}

impl<S: MailstromStorage + 'static> Drop for Mailstrom<S>
{
    fn drop(&mut self) {
        let _ = self.sender.send(Message::Terminate);
    }
}
