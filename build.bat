@echo off
REM Builds dist\Jype.exe  (run this on Windows 10/11)
setlocal
cd /d "%~dp0"

where cargo >nul 2>nul
if errorlevel 1 (
  echo.
  echo Rust is not installed.
  echo 1^) Install Rust from https://rustup.rs   ^(accept the defaults^)
  echo 2^) If asked, also install "Visual Studio Build Tools" with the "Desktop development with C++" workload.
  echo 3^) Close this window and run build.bat again.
  echo.
  pause
  exit /b 1
)

echo Running the typing-engine tests...
cargo test -p bangla-engine --release
if errorlevel 1 (
  echo.
  echo Tests failed - please send me the output above.
  pause
  exit /b 1
)

echo Building Jype...
cargo build --release -p jype
if errorlevel 1 (
  echo.
  echo Build failed - please send me the output above.
  pause
  exit /b 1
)

if exist dist rmdir /s /q dist
mkdir dist
copy /y target\release\jype.exe dist\Jype.exe >nul
xcopy /e /i /q LICENSES dist\LICENSES >nul
copy /y THIRD_PARTY_NOTICES.md dist\ >nul

echo.
echo Done!  ->  dist\Jype.exe
echo Double-click it. A green icon appears near the clock.
pause
