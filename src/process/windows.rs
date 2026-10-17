//! Confirm owned Windows descendants have exited, not just a completion-port
//! notification. process-wrap 10.0.1's wait helper accepts any job notification
//! and caches the leader's exit before establishing that the whole job is empty.
use ::windows::{
    Win32::{
        Foundation::{ERROR_INVALID_PARAMETER, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::{
            JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, IsProcessInJob,
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
                JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectBasicAccountingInformation,
                JobObjectBasicProcessIdList, JobObjectExtendedLimitInformation,
                QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
            },
            Threading::{
                OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
                WaitForSingleObject,
            },
        },
    },
    core::{BOOL, PCWSTR},
};
use process_wrap::tokio::{ChildWrapper, CommandWrap, CommandWrapper};
use std::{
    io::{Error, Result},
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::process::{Child, Command};

#[derive(Debug)]
pub(super) struct Job {
    handle: OwnedHandle,
    pending: Mutex<Option<Arc<Vec<OwnedHandle>>>>,
}

impl Job {
    pub(super) fn new() -> Result<Arc<Self>> {
        // Unnamed, non-inheritable, application-owned handle: no PID lookup or
        // access to the runner's/another application's inherited outer job.
        let handle = unsafe { CreateJobObjectW(None, PCWSTR::null()) }.map_err(Error::other)?;
        // SAFETY: CreateJobObjectW returned a fresh handle; this is its sole owner.
        let job = Self {
            handle: unsafe { OwnedHandle::from_raw_handle(handle.0) },
            pending: Mutex::new(None),
        };
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
        HANDLE(self.handle.as_raw_handle())
    }

    pub(super) fn terminate(&self) -> Result<()> {
        // Capture before the FIRST termination request, including cancellation.
        // An already-terminating process can disappear from job accounting
        // before its native process object is signalled. Retain these handles
        // on the job so dropping/re-polling a wait cannot lose that evidence.
        let snapshot = (|| {
            let mut pending = self
                .pending
                .lock()
                .map_err(|_| Error::other("Windows cleanup handle lock failed."))?;
            if pending.is_none() {
                *pending = Some(Arc::new(self.members()?));
            }
            Ok(())
        })();
        // SAFETY: this private job contains only the suspended child assigned
        // below and its descendants. The owned handle cannot be reused here.
        // Even if evidence capture failed, attempt termination; the error still
        // prevents cleanup being reported safe.
        unsafe { TerminateJobObject(self.handle(), 1) }.map_err(Error::other)?;
        snapshot
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

    fn members(&self) -> Result<Vec<OwnedHandle>> {
        // SDK variable-length JOBOBJECT_BASIC_PROCESS_ID_LIST layout. A larger
        // tree fails closed with ERROR_MORE_DATA rather than allocating freely.
        #[repr(C)]
        struct Processes {
            assigned: u32,
            count: u32,
            ids: [usize; 4096],
        }
        let mut info = Processes {
            assigned: 0,
            count: 0,
            ids: [0; 4096],
        };
        // SAFETY: live private job and aligned, matching variable-length buffer.
        unsafe {
            QueryInformationJobObject(
                Some(self.handle()),
                JobObjectBasicProcessIdList,
                std::ptr::from_mut(&mut info).cast(),
                std::mem::size_of_val(&info) as u32,
                None,
            )
        }
        .map_err(Error::other)?;
        if info.count != info.assigned {
            return Err(Error::other(
                "Windows job process list exceeded its cleanup bound.",
            ));
        }
        let ids = info
            .ids
            .get(..info.count as usize)
            .ok_or_else(|| Error::other("Invalid Windows job process count."))?;
        let mut members = Vec::with_capacity(ids.len());
        for &pid in ids {
            let pid = u32::try_from(pid).map_err(Error::other)?;
            // No PID is ever signalled. Hold a read-only native handle before
            // terminating the private job, to await actual process signalling.
            let handle = match unsafe {
                OpenProcess(
                    PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
                    false,
                    pid,
                )
            } {
                Ok(handle) => unsafe { OwnedHandle::from_raw_handle(handle.0) },
                Err(error) if error.code() == ERROR_INVALID_PARAMETER.to_hresult() => continue,
                Err(error) => return Err(Error::other(error)),
            };
            let mut member = BOOL::default();
            // SAFETY: both handles remain owned and the BOOL buffer is valid.
            unsafe {
                IsProcessInJob(
                    HANDLE(handle.as_raw_handle()),
                    Some(self.handle()),
                    &mut member,
                )
            }
            .map_err(Error::other)?;
            // The listed process may have exited and its PID been reused.
            // Such an unrelated handle grants no cleanup authority.
            if member.as_bool() {
                members.push(handle);
            }
        }
        Ok(members)
    }

    pub(super) async fn finish(&self) -> Result<()> {
        // Match Unix ownership: once the leader exits, remaining attached
        // descendants cannot keep writing after this job releases its lease.
        // Always terminate the job, including its nested jobs; an accounting
        // observation alone must never skip the owned termination request.
        self.terminate()?;
        let members = self
            .pending
            .lock()
            .map_err(|_| Error::other("Windows cleanup handle lock failed."))?
            .clone()
            .ok_or_else(|| Error::other("Missing Windows cleanup process handles."))?;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let mut pending = false;
            for process in members.iter() {
                // SAFETY: retained owned process handle with SYNCHRONIZE access.
                match unsafe { WaitForSingleObject(HANDLE(process.as_raw_handle()), 0) } {
                    WAIT_OBJECT_0 => {}
                    WAIT_TIMEOUT => pending = true,
                    _ => return Err(Error::last_os_error()),
                }
            }
            if !pending {
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(Error::other("Windows job member termination timed out."));
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        // Native handles must be released before checking final job accounting.
        drop(members);
        drop(
            self.pending
                .lock()
                .map_err(|_| Error::other("Windows cleanup handle lock failed."))?
                .take(),
        );
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
