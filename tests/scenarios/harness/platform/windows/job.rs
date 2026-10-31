//! A run's processes on Windows, which has no process group and no process table to read: a
//! job object holds a run and everything it starts, and the system's process list names them
//! (S24).

use std::ffi::c_void;
use std::io;
use std::mem;
use std::process::{Child, Command};
use std::ptr;

use super::{AsRawHandle, CommandExt, FromRawHandle, Handle, OwnedHandle};

const CREATE_SUSPENDED: u32 = 0x4;
const ERROR_INVALID_PARAMETER: i32 = 87;
const INVALID_HANDLE_VALUE: Handle = -1isize as Handle;
const JOB_OBJECT_BASIC_PROCESS_ID_LIST: i32 = 3;
const SYNCHRONIZE: u32 = 0x0010_0000;
const TH32CS_SNAPPROCESS: u32 = 0x2;
const TH32CS_SNAPTHREAD: u32 = 0x4;
const THREAD_SUSPEND_RESUME: u32 = 0x2;
const WAIT_OBJECT_0: u32 = 0;
/// The processes of a job one listing holds: far more than a run of Git starts.
const JOB_LIST: usize = 256;
/// The status a killed job's processes end with, the one a killed Unix process's shell shows.
const KILLED: u32 = 137;

#[repr(C)]
struct ThreadEntry {
    size: u32,
    usage: u32,
    thread: u32,
    owner: u32,
    base_priority: i32,
    delta_priority: i32,
    flags: u32,
}

#[repr(C)]
struct ProcessEntry {
    size: u32,
    usage: u32,
    process: u32,
    heap: usize,
    module: u32,
    threads: u32,
    parent: u32,
    class_priority: i32,
    flags: u32,
    executable: [u16; 260],
}

#[repr(C)]
struct JobProcessList {
    assigned: u32,
    listed: u32,
    processes: [usize; JOB_LIST],
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
    fn CreateJobObjectW(attributes: *const c_void, name: *const u16) -> Handle;
    fn CreateToolhelp32Snapshot(flags: u32, process: u32) -> Handle;
    fn OpenProcess(access: u32, inherit: i32, process: u32) -> Handle;
    fn OpenThread(access: u32, inherit: i32, thread: u32) -> Handle;
    fn Process32FirstW(snapshot: Handle, entry: *mut ProcessEntry) -> i32;
    fn Process32NextW(snapshot: Handle, entry: *mut ProcessEntry) -> i32;
    fn QueryInformationJobObject(
        job: Handle,
        class: i32,
        information: *mut c_void,
        length: u32,
        returned: *mut u32,
    ) -> i32;
    fn ResumeThread(thread: Handle) -> u32;
    fn TerminateJobObject(job: Handle, code: u32) -> i32;
    fn Thread32First(snapshot: Handle, entry: *mut ThreadEntry) -> i32;
    fn Thread32Next(snapshot: Handle, entry: *mut ThreadEntry) -> i32;
    fn WaitForSingleObject(handle: Handle, milliseconds: u32) -> u32;
}

/// The handle a call answered, owned, or the system's error where it answered `failed`.
fn owned(handle: Handle, failed: Handle) -> io::Result<OwnedHandle> {
    if handle == failed {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the call that returned `handle` gave this process its one reference.
    Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
}

/// A job object holding a run and everything it starts.
pub struct Group {
    job: OwnedHandle,
}

impl Group {
    /// The processes of the job that have not ended, each its identifier and its name from
    /// the system's process list. The job's one listing decides who runs: the job lists no
    /// process that has exited, and a process the later snapshot does not hold is still one
    /// that ran when the job was asked, so that a child outliving its parent is never missed.
    pub fn running(&self) -> Result<Vec<String>, String> {
        let listed = self
            .listed()
            .map_err(|cause| format!("the job's processes: {cause}"))?;
        let names = process_names().map_err(|cause| format!("the process list: {cause}"))?;
        let name = |process: u32| {
            let found = names.iter().find(|(named, _)| *named == process);
            found.map_or("?", |(_, name)| name.as_str()).to_owned()
        };
        let rows = listed
            .into_iter()
            .map(|process| format!("{process} {}", name(process)));
        Ok(rows.collect())
    }

    /// Ends every process of the job at once.
    pub fn kill(&self) {
        // SAFETY: the job's handle is open for the call.
        unsafe { TerminateJobObject(self.job.as_raw_handle(), KILLED) };
    }

    /// The identifiers the job lists.
    fn listed(&self) -> io::Result<Vec<u32>> {
        // SAFETY: all zeroes is a list of no process, and the call writes within its size.
        let mut list: JobProcessList = unsafe { mem::zeroed() };
        let asked = unsafe {
            QueryInformationJobObject(
                self.job.as_raw_handle(),
                JOB_OBJECT_BASIC_PROCESS_ID_LIST,
                (&raw mut list).cast(),
                mem::size_of::<JobProcessList>() as u32,
                ptr::null_mut(),
            )
        };
        if asked == 0 {
            return Err(io::Error::last_os_error());
        }
        let listed = list.processes[..list.listed as usize].iter();
        Ok(listed.map(|process| *process as u32).collect())
    }
}

/// Starts `command` in a job of its own: made suspended, assigned, and resumed, so that
/// nothing it starts escapes the job. `Child` gives no thread handle, so its one thread is
/// found in the system's list. A child that cannot be so placed is killed, never left
/// suspended.
pub fn spawn_grouped(command: &mut Command) -> io::Result<(Child, Group)> {
    command.creation_flags(CREATE_SUSPENDED);
    let mut child = command.spawn()?;
    match assigned_and_resumed(&child) {
        Ok(job) => Ok((child, Group { job })),
        Err(cause) => {
            let _ = child.kill();
            let _ = child.wait();
            Err(cause)
        }
    }
}

/// A new job holding the suspended `child`, which then runs.
fn assigned_and_resumed(child: &Child) -> io::Result<OwnedHandle> {
    // SAFETY: null asks for default attributes and no name.
    let job = owned(
        unsafe { CreateJobObjectW(ptr::null(), ptr::null()) },
        ptr::null_mut(),
    )?;
    // SAFETY: both handles are open for the call.
    if unsafe { AssignProcessToJobObject(job.as_raw_handle(), child.as_raw_handle()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let thread = thread_of(child.id())?;
    // SAFETY: the system's answer is the access and the identifier asked for.
    let thread = owned(
        unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, thread) },
        ptr::null_mut(),
    )?;
    // SAFETY: the handle is open for the call.
    if unsafe { ResumeThread(thread.as_raw_handle()) } == u32::MAX {
        return Err(io::Error::last_os_error());
    }
    Ok(job)
}

/// The identifier of the first thread the system lists for `process`, a suspended child's
/// one thread.
fn thread_of(process: u32) -> io::Result<u32> {
    // SAFETY: the snapshot takes no pointer.
    let snapshot = owned(
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) },
        INVALID_HANDLE_VALUE,
    )?;
    let mut entry = ThreadEntry {
        size: mem::size_of::<ThreadEntry>() as u32,
        usage: 0,
        thread: 0,
        owner: 0,
        base_priority: 0,
        delta_priority: 0,
        flags: 0,
    };
    // SAFETY: `entry` carries its size and the snapshot is open for each call.
    let mut more = unsafe { Thread32First(snapshot.as_raw_handle(), &mut entry) };
    while more != 0 {
        if entry.owner == process {
            return Ok(entry.thread);
        }
        // SAFETY: as above.
        more = unsafe { Thread32Next(snapshot.as_raw_handle(), &mut entry) };
    }
    Err(io::Error::other(format!("process {process} has no thread")))
}

/// Every process of the system by its identifier, with its name.
fn process_names() -> io::Result<Vec<(u32, String)>> {
    // SAFETY: the snapshot takes no pointer.
    let snapshot = owned(
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) },
        INVALID_HANDLE_VALUE,
    )?;
    // SAFETY: all zeroes is a valid entry, and each call below is given its size.
    let mut entry: ProcessEntry = unsafe { mem::zeroed() };
    entry.size = mem::size_of::<ProcessEntry>() as u32;
    let mut names = Vec::new();
    // SAFETY: `entry` carries its size and the snapshot is open for each call.
    let mut more = unsafe { Process32FirstW(snapshot.as_raw_handle(), &mut entry) };
    while more != 0 {
        let ends = entry.executable.iter().position(|unit| *unit == 0);
        let spelled = &entry.executable[..ends.unwrap_or(entry.executable.len())];
        names.push((entry.process, String::from_utf16_lossy(spelled)));
        // SAFETY: as above.
        more = unsafe { Process32NextW(snapshot.as_raw_handle(), &mut entry) };
    }
    Ok(names)
}

/// A process whose number stays its own: Windows gives a number that fell free to the next
/// process at once, so that `ended` of a number can turn false again, and a handle keeps it.
pub struct HeldProcess(Option<OwnedHandle>);

impl HeldProcess {
    /// Whether the process has ended.
    pub fn ended(&self) -> bool {
        self.0.as_ref().is_none_or(|handle| {
            // SAFETY: the handle is open for the call, which waits for nothing.
            unsafe { WaitForSingleObject(handle.as_raw_handle(), 0) == WAIT_OBJECT_0 }
        })
    }
}

/// `process`, which runs, held for as long as the answer lives; a process already gone
/// answers that it has ended.
pub fn process_held(process: &str) -> HeldProcess {
    let number = process
        .parse()
        .unwrap_or_else(|cause| panic!("process {process:?}: {cause}"));
    // SAFETY: the call takes no pointer.
    let handle = unsafe { OpenProcess(SYNCHRONIZE, 0, number) };
    HeldProcess(owned(handle, ptr::null_mut()).ok())
}

/// Whether the process `process` has ended: gone, or signaled while a handle keeps it. The
/// signaled state and never the exit code, because a process can exit with `STILL_ACTIVE`'s
/// value.
pub fn ended(process: &str) -> bool {
    let number = process
        .parse()
        .unwrap_or_else(|cause| panic!("process {process:?}: {cause}"));
    // SAFETY: the call takes no pointer.
    let handle = unsafe { OpenProcess(SYNCHRONIZE, 0, number) };
    let opened = match owned(handle, ptr::null_mut()) {
        Ok(opened) => opened,
        Err(cause) if cause.raw_os_error() == Some(ERROR_INVALID_PARAMETER) => return true,
        // A process this session may not open is running.
        Err(_) => return false,
    };
    // SAFETY: the handle is open for the call, which waits for nothing.
    unsafe { WaitForSingleObject(opened.as_raw_handle(), 0) == WAIT_OBJECT_0 }
}
