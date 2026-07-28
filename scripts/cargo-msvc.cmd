@echo off
setlocal
set "VSDEVCMD="
for /f "usebackq tokens=*" %%i in (`"%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -find Common7\Tools\VsDevCmd.bat`) do set "VSDEVCMD=%%i"
if not defined VSDEVCMD (
  echo Visual Studio C++ build tools were not found. 1>&2
  exit /b 1
)
call "%VSDEVCMD%" -arch=x64 -host_arch=x64 >nul
if errorlevel 1 exit /b %errorlevel%
set "PATH=%~dp0..\.tools\strawberry-perl\perl\bin;%PATH%"
cargo %*
