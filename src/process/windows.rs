//! Confirm owned Windows descendants have exited, not just a completion-port
//! notification. process-wrap 10.0.1's wait helper accepts any job notification
//! and caches the leader's exit before establishing that the whole job is empty.
use ::windows::{
    Win32::{
        Foundation::HANDLE,
        System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JobObjectBasicAccountingInformation, JobObjectExtendedLimitInformation,
            QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
        },
    },
    core::PCWSTR,
};
use process_wrap::tokio::{ChildWrapper, CommandWrap, CommandWrapper};
use std::{
    io::{Error, Result},
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    sync::Arc,
    time::Duration,
};
use tokio::process::{Child, Command};

#[derive(Debug)]
pub(super) struct Job(OwnedHandle);

impl Job {
    pub(super) fn new() -> Result<Arc<Self>> {
        // Unnamed, non-inheritable, application-owned handle: no PID lookup or
        // access to the runner's/another application's inherited outer job.
        let handle = unsafe { CreateJobObjectW(None, PCWSTR::null()) }.map_err(Error::other)?;
        // SAFETY: CreateJobObjectW returned a fresh handle; this is its sole owner.
        let job = Self(unsafe { OwnedHandle::from_raw_handle(handle.0) });
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: owned live job and the SDK's fixed-layout structure/size.
        unsafe {
            SetInformationJobObject(
                job.handle(),
                JobObjectExtendedLimitInformation,
                std::ptr::from_ref(&limits).cast(),
                std::mem::size_of_val(&limits) as u32,
            )
        }
        .map_err(Error::other)?;
        Ok(Arc::new(job))
    }

    fn handle(&self) -> HANDLE {
        HANDLE(self.0.as_raw_handle())
    }

    pub(super) fn terminate(&self) -> Result<()> {
        // SAFETY: this private job contains only the suspended child assigned
        // below and its descendants. The owned handle cannot be reused here.
        unsafe { TerminateJobObject(self.handle(), 1) }.map_err(Error::other)
    }

    fn active(&self) -> Result<u32> {
        let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        // SAFETY: live owned handle and matching writable SDK buffer/size.
        unsafe {
            QueryInformationJobObject(
                Some(self.handle()),
                JobObjectBasicAccountingInformation,
                std::ptr::from_mut(&mut info).cast(),
                std::mem::size_of_val(&info) as u32,
                None,
            )
        }
        .map_err(Error::other)?;
        Ok(info.ActiveProcesses)
    }

    pub(super) async fn finish(&self) -> Result<()> {
        if self.active()? == 0 {
            return Ok(());
        }
        // Match Unix ownership: once the leader exits, remaining attached
        // descendants cannot keep writing after this job releases its lease.
        self.terminate()?;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            if self.active()? == 0 {
                return Ok(());
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(Error::other(
                    "Windows Job Object cleanup timed out; repository protection must remain reserved.",
                ));
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
}

#[derive(Debug)]
pub(super) struct Assign(pub(super) Arc<Job>);

impl CommandWrapper for Assign {
    fn post_spawn(&mut self, _: &mut Command, child: &mut Child, _: &CommandWrap) -> Result<()> {
        // All pre_spawn hooks have run: JobObject set CREATE_SUSPENDED. All
        // post_spawn hooks precede JobObject::wrap_child, which resumes it.
        // This outer job therefore contains every descendant without a race.
        let process = child
            .process_handle()
            .ok_or_else(|| Error::other("Missing owned Windows process handle."))?;
        // SAFETY: both handles are borrowed from live owned Windows objects.
        unsafe { AssignProcessToJobObject(self.0.handle(), HANDLE(process.as_raw_handle())) }
            .map_err(Error::other)
    }
}
