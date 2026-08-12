Set-StrictMode -Version Latest

if (-not ('MistakeTrainer.JobObjects' -as [type])) {
  Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;

namespace MistakeTrainer {
  public static class JobObjects {
    const uint CREATE_SUSPENDED = 0x00000004;
    const uint JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE = 0x00002000;
    const uint JOB_OBJECT_LIMIT_BREAKAWAY_OK = 0x00000800;
    const int JobObjectExtendedLimitInformation = 9;

    [StructLayout(LayoutKind.Sequential)] struct JOBOBJECT_BASIC_LIMIT_INFORMATION {
      public long PerProcessUserTimeLimit, PerJobUserTimeLimit;
      public uint LimitFlags;
      public UIntPtr MinimumWorkingSetSize, MaximumWorkingSetSize;
      public uint ActiveProcessLimit;
      public UIntPtr Affinity;
      public uint PriorityClass, SchedulingClass;
    }
    [StructLayout(LayoutKind.Sequential)] struct IO_COUNTERS {
      public ulong ReadOperationCount, WriteOperationCount, OtherOperationCount;
      public ulong ReadTransferCount, WriteTransferCount, OtherTransferCount;
    }
    [StructLayout(LayoutKind.Sequential)] struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
      public JOBOBJECT_BASIC_LIMIT_INFORMATION BasicLimitInformation;
      public IO_COUNTERS IoInfo;
      public UIntPtr ProcessMemoryLimit, JobMemoryLimit, PeakProcessMemoryUsed, PeakJobMemoryUsed;
    }
    [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] struct STARTUPINFO {
      public int cb; public string lpReserved, lpDesktop, lpTitle;
      public uint dwX, dwY, dwXSize, dwYSize, dwXCountChars, dwYCountChars, dwFillAttribute, dwFlags;
      public short wShowWindow, cbReserved2; public IntPtr lpReserved2, hStdInput, hStdOutput, hStdError;
    }
    [StructLayout(LayoutKind.Sequential)] struct PROCESS_INFORMATION {
      public IntPtr hProcess, hThread; public uint dwProcessId, dwThreadId;
    }

    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern IntPtr CreateJobObjectW(IntPtr a, string n);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool SetInformationJobObject(IntPtr j, int c, IntPtr i, uint l);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern bool CreateProcessW(string app, StringBuilder cmd, IntPtr pa, IntPtr ta, bool inherit, uint flags, IntPtr env, string cwd, ref STARTUPINFO si, out PROCESS_INFORMATION pi);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool AssignProcessToJobObject(IntPtr j, IntPtr p);
    [DllImport("kernel32.dll", SetLastError=true)] static extern uint ResumeThread(IntPtr t);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool TerminateProcess(IntPtr p, uint c);
    [DllImport("kernel32.dll", SetLastError=true)] public static extern bool CloseHandle(IntPtr h);

    static void Fail(string operation) { throw new Win32Exception(Marshal.GetLastWin32Error(), operation); }

    public static IntPtr CreateKillOnClose(bool allowBreakaway) {
      IntPtr job = CreateJobObjectW(IntPtr.Zero, null); if (job == IntPtr.Zero) Fail("CreateJobObjectW");
      var info = new JOBOBJECT_EXTENDED_LIMIT_INFORMATION();
      info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE |
        (allowBreakaway ? JOB_OBJECT_LIMIT_BREAKAWAY_OK : 0);
      int size = Marshal.SizeOf(info); IntPtr ptr = Marshal.AllocHGlobal(size);
      try {
        Marshal.StructureToPtr(info, ptr, false);
        if (!SetInformationJobObject(job, JobObjectExtendedLimitInformation, ptr, (uint)size)) { CloseHandle(job); Fail("SetInformationJobObject"); }
      } finally { Marshal.FreeHGlobal(ptr); }
      return job;
    }

    static string Quote(string value) {
      if (value.Length > 0 && value.IndexOfAny(new[]{' ', '\t', '"'}) < 0) return value;
      var b = new StringBuilder("\""); int slashes = 0;
      foreach (char c in value) {
        if (c == '\\') { slashes++; continue; }
        if (c == '"') { b.Append('\\', slashes * 2 + 1).Append(c); slashes = 0; continue; }
        b.Append('\\', slashes).Append(c); slashes = 0;
      }
      b.Append('\\', slashes * 2).Append('"'); return b.ToString();
    }
    public static string QuoteCommandLine(string file, string[] args) {
      var b = new StringBuilder(Quote(file)); foreach (string arg in args) b.Append(' ').Append(Quote(arg ?? "")); return b.ToString();
    }
    public static int StartAssigned(IntPtr job, string file, string commandLine) {
      var si = new STARTUPINFO(); si.cb = Marshal.SizeOf(si); PROCESS_INFORMATION pi;
      if (!CreateProcessW(file, new StringBuilder(commandLine), IntPtr.Zero, IntPtr.Zero, false, CREATE_SUSPENDED, IntPtr.Zero, null, ref si, out pi)) Fail("CreateProcessW");
      try {
        if (!AssignProcessToJobObject(job, pi.hProcess)) { TerminateProcess(pi.hProcess, 125); Fail("AssignProcessToJobObject"); }
        if (ResumeThread(pi.hThread) == 0xffffffff) { TerminateProcess(pi.hProcess, 126); Fail("ResumeThread"); }
        return checked((int)pi.dwProcessId);
      } finally { CloseHandle(pi.hThread); CloseHandle(pi.hProcess); }
    }
  }
}
'@
}

function New-KillOnCloseJob {
  param([switch]$AllowBreakaway)
  [pscustomobject]@{ Handle = [MistakeTrainer.JobObjects]::CreateKillOnClose($AllowBreakaway.IsPresent); Closed = $false }
}

function Start-ProcessInJob {
  param([Parameter(Mandatory)]$Job, [Parameter(Mandatory)][string]$FilePath, [string[]]$ArgumentList = @())
  if ($Job.Closed) { throw 'Job Object is already closed.' }
  $commandLine = [MistakeTrainer.JobObjects]::QuoteCommandLine($FilePath, $ArgumentList)
  $processId = [MistakeTrainer.JobObjects]::StartAssigned($Job.Handle, $FilePath, $commandLine)
  [System.Diagnostics.Process]::GetProcessById($processId)
}

function Wait-JobProcessExit {
  param([Parameter(Mandatory)][System.Diagnostics.Process]$Process, [int]$TimeoutSeconds = 30)
  return $Process.WaitForExit($TimeoutSeconds * 1000)
}

function Close-KillOnCloseJob {
  param([Parameter(Mandatory)]$Job)
  if (-not $Job.Closed) {
    [void][MistakeTrainer.JobObjects]::CloseHandle($Job.Handle)
    $Job.Closed = $true
  }
}
